// Synthetic host integration only: no registration, source selection, system
// key injection, foreground changes, clipboard access or other application's text.
// Real Windows TSF contexts/ACP locks are used; the manager shim owns only the
// tested TIP's event subscriptions. This does not validate system profile loading.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <msctf.h>
#include <ctffunc.h>
#include <textstor.h>
#include <olectl.h>
#include <wrl/client.h>
#include <algorithm>
#include <cctype>
#include <cwchar>
#include <deque>
#include <functional>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
using Microsoft::WRL::ComPtr;

static void check(HRESULT hr, const char* stage) {
    if (FAILED(hr)) {
        char message[160];
        sprintf_s(message, "%s HRESULT=0x%08lX", stage, static_cast<unsigned long>(hr));
        throw std::runtime_error(message);
    }
}
static void require(bool condition, const char* stage) {
    if (!condition) throw std::runtime_error(stage);
}
static unsigned pump(DWORD milliseconds, HWND observed_mouse_window = nullptr) {
    const auto deadline = GetTickCount64() + milliseconds;
    unsigned processed = 0;
    unsigned mouse_leaves = 0;
    unsigned overloaded_rounds = 0;
    do {
        MSG message{};
        const auto round_started = GetTickCount64();
        unsigned round_count = 0;
        while (round_count < 256 && GetTickCount64() - round_started < 8
            && PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            ++round_count;
            if (++processed > 8192)
                throw std::runtime_error("Fixture message flood exceeds total processing budget");
            if (message.hwnd == observed_mouse_window && message.message == WM_MOUSELEAVE)
                ++mouse_leaves;
            // The test delivers virtual keys through the owned sink. Actual key
            // messages must never become text in the synthetic store.
            if (message.message >= WM_KEYFIRST && message.message <= WM_KEYLAST)
                throw std::runtime_error("Unexpected physical key message in fixture thread");
            if (message.message != WM_QUIT) DispatchMessageW(&message);
        }
        if (round_count == 256 || GetTickCount64() - round_started >= 8) {
            if (++overloaded_rounds >= 16)
                throw std::runtime_error("Fixture message queue stays overloaded across bounded rounds");
        } else {
            overloaded_rounds = 0;
        }
        if (GetTickCount64() < deadline) Sleep(2);
    } while (GetTickCount64() < deadline);
    return mouse_leaves;
}

static BOOL CALLBACK find_owned_candidates(HWND window, LPARAM output) {
    wchar_t class_name[80]{};
    GetClassNameW(window, class_name, static_cast<int>(sizeof(class_name) / sizeof(*class_name)));
    constexpr wchar_t prefix[] = L"LingoMate.Candidates.";
    if (std::wcsncmp(class_name, prefix, sizeof(prefix) / sizeof(*prefix) - 1) == 0
        && IsWindowVisible(window)) {
        *reinterpret_cast<HWND*>(output) = window;
        return FALSE;
    }
    return TRUE;
}

static HWND owned_candidates() {
    HWND window = nullptr;
    EnumThreadWindows(GetCurrentThreadId(), find_owned_candidates, reinterpret_cast<LPARAM>(&window));
    if (window) {
        DWORD process = 0;
        require(GetWindowThreadProcessId(window, &process) == GetCurrentThreadId()
            && process == GetCurrentProcessId(), "Candidate fixture must own the exact window and thread");
    }
    return window;
}

class FixtureStore final : public ITextStoreACP, public ITfContextOwnerCompositionSink {
    ULONG references = 1;
    DWORD lock = 0;
    DWORD pending_lock = 0;
    ComPtr<ITextStoreACPSink> sink;
    TS_SELECTION_ACP selection{0, 0, {TS_AE_END, FALSE}};
    HWND window;
    bool readable() const { return (lock & TS_LF_READ) != 0; }
    bool writable() const { return (lock & TS_LF_READWRITE) == TS_LF_READWRITE; }
public:
    std::wstring text;
    std::vector<std::wstring> ended;
    unsigned started = 0;
    explicit FixtureStore(HWND hwnd) : window(hwnd) {}
    unsigned count(const std::wstring& value) const {
        return static_cast<unsigned>(std::count(ended.begin(), ended.end(), value));
    }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** output) override {
        if (!output) return E_POINTER;
        *output = nullptr;
        if (iid == IID_IUnknown || iid == IID_ITextStoreACP)
            *output = static_cast<ITextStoreACP*>(this);
        else if (iid == IID_ITfContextOwnerCompositionSink)
            *output = static_cast<ITfContextOwnerCompositionSink*>(this);
        else return E_NOINTERFACE;
        AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++references; }
    ULONG STDMETHODCALLTYPE Release() override {
        const ULONG remaining = --references;
        if (!remaining) delete this;
        return remaining;
    }
    HRESULT STDMETHODCALLTYPE AdviseSink(REFIID iid, IUnknown* unknown, DWORD) override {
        if (iid != IID_ITextStoreACPSink || !unknown) return E_INVALIDARG;
        ComPtr<ITextStoreACPSink> candidate;
        const auto result = unknown->QueryInterface(IID_PPV_ARGS(&candidate));
        if (FAILED(result)) return result;
        if (sink && sink.Get() != candidate.Get()) return CONNECT_E_ADVISELIMIT;
        sink = candidate; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE UnadviseSink(IUnknown* unknown) override {
        ComPtr<ITextStoreACPSink> candidate;
        if (!unknown || FAILED(unknown->QueryInterface(IID_PPV_ARGS(&candidate))) || candidate != sink)
            return CONNECT_E_NOCONNECTION;
        sink.Reset(); return S_OK;
    }
    HRESULT STDMETHODCALLTYPE RequestLock(DWORD flags, HRESULT* session) override {
        if (!sink || !session) return E_INVALIDARG;
        if (lock) {
            if (flags & TS_LF_SYNC) { *session = TS_E_SYNCHRONOUS; return S_OK; }
            // A read-only holder may request an asynchronous write upgrade.
            // Grant it only after OnLockGranted returns; never downgrade pending work.
            pending_lock |= flags & TS_LF_READWRITE;
            *session = TS_S_ASYNC; return S_OK;
        }
        lock = flags;
        *session = sink->OnLockGranted(flags);
        lock = 0;
        while (pending_lock) {
            const DWORD next = pending_lock;
            pending_lock = 0;
            lock = next;
            sink->OnLockGranted(next);
            lock = 0;
        }
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetStatus(TS_STATUS* output) override {
        if (!output) return E_POINTER;
        *output = {0, TS_SS_NOHIDDENTEXT}; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE QueryInsert(LONG a, LONG b, ULONG, LONG* start, LONG* end) override {
        if (!start || !end) return E_POINTER;
        if (a < 0 || b < a || b > static_cast<LONG>(text.size())) return TS_E_INVALIDPOS;
        *start = a; *end = b; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetSelection(ULONG index, ULONG count, TS_SELECTION_ACP* output, ULONG* fetched) override {
        if (!readable()) return TS_E_NOLOCK;
        if (!fetched || (count && !output)) return E_POINTER;
        if (index != TS_DEFAULT_SELECTION && index != 0) return TS_E_NOSELECTION;
        *fetched = count ? 1 : 0;
        if (count) *output = selection;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetSelection(ULONG count, const TS_SELECTION_ACP* value) override {
        if (!writable()) return TS_E_NOLOCK;
        if (count != 1 || !value) return E_INVALIDARG;
        if (value->acpStart < 0 || value->acpEnd < value->acpStart || value->acpEnd > static_cast<LONG>(text.size())) return TS_E_INVALIDPOS;
        selection = *value; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetText(LONG start, LONG end, WCHAR* output, ULONG requested,
        ULONG* returned, TS_RUNINFO* runs, ULONG requested_runs, ULONG* returned_runs, LONG* next) override {
        if (!readable()) return TS_E_NOLOCK;
        if (!returned || !returned_runs || !next || (requested && !output) || (requested_runs && !runs)) return E_POINTER;
        if (end == -1) end = static_cast<LONG>(text.size());
        if (start < 0 || end < start || end > static_cast<LONG>(text.size())) return TS_E_INVALIDPOS;
        const ULONG span = static_cast<ULONG>(end - start);
        const ULONG plain = std::min(requested, span);
        if (plain) std::copy_n(text.data() + start, plain, output);
        // TSF can request only run information, without a plain-text buffer.
        const ULONG run_chars = requested ? plain : span;
        *returned = plain;
        *returned_runs = requested_runs && run_chars ? 1 : 0;
        if (*returned_runs) runs[0] = {run_chars, TS_RT_PLAIN};
        *next = start + (requested ? plain : (requested_runs ? run_chars : 0));
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetText(DWORD, LONG start, LONG end, const WCHAR* value,
        ULONG count, TS_TEXTCHANGE* change) override {
        if (!writable()) return TS_E_NOLOCK;
        if (!change || (count && !value)) return E_POINTER;
        if (start < 0 || end < start || end > static_cast<LONG>(text.size())) return TS_E_INVALIDPOS;
        text.replace(start, end - start, value ? value : L"", count);
        *change = {start, end, start + static_cast<LONG>(count)};
        selection.acpStart = selection.acpEnd = change->acpNewEnd;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetFormattedText(LONG, LONG, IDataObject**) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetEmbedded(LONG, REFGUID, REFIID, IUnknown**) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE QueryInsertEmbedded(const GUID*, const FORMATETC*, BOOL* output) override {
        if (!output) return E_POINTER;
        *output = FALSE; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE InsertEmbedded(DWORD, LONG, LONG, IDataObject*, TS_TEXTCHANGE*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE InsertTextAtSelection(DWORD flags, const WCHAR* value, ULONG count,
        LONG* start, LONG* end, TS_TEXTCHANGE* change) override {
        if (!writable()) return TS_E_NOLOCK;
        const LONG a = selection.acpStart, b = selection.acpEnd;
        if (start) *start = a;
        if (end) *end = (flags & TS_IAS_QUERYONLY) ? b : a + static_cast<LONG>(count);
        if (flags & TS_IAS_QUERYONLY) return S_OK;
        TS_TEXTCHANGE local;
        return SetText(0, a, b, value, count, change ? change : &local);
    }
    HRESULT STDMETHODCALLTYPE InsertEmbeddedAtSelection(DWORD, IDataObject*, LONG*, LONG*, TS_TEXTCHANGE*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE RequestSupportedAttrs(DWORD, ULONG, const TS_ATTRID*) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestAttrsAtPosition(LONG, ULONG, const TS_ATTRID*, DWORD) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestAttrsTransitioningAtPosition(LONG, ULONG, const TS_ATTRID*, DWORD) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE FindNextAttrTransition(LONG, LONG halt, ULONG, const TS_ATTRID*, DWORD,
        LONG* next, BOOL* found, LONG* offset) override { *next = halt; *found = FALSE; *offset = 0; return S_OK; }
    HRESULT STDMETHODCALLTYPE RetrieveRequestedAttrs(ULONG, TS_ATTRVAL*, ULONG* count) override { *count = 0; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetEndACP(LONG* output) override {
        if (!readable()) return TS_E_NOLOCK;
        if (!output) return E_POINTER;
        *output = static_cast<LONG>(text.size()); return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetActiveView(TsViewCookie* output) override { *output = 1; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetACPFromPoint(TsViewCookie, const POINT*, DWORD, LONG* output) override {
        *output = static_cast<LONG>(text.size()); return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetTextExt(TsViewCookie, LONG, LONG, RECT* output, BOOL* clipped) override {
        *output = {-32000, -32000, -31800, -31975}; *clipped = FALSE; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetScreenExt(TsViewCookie, RECT* output) override {
        *output = {-32000, -32000, -31360, -31520}; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetWnd(TsViewCookie, HWND* output) override { *output = window; return S_OK; }
    HRESULT STDMETHODCALLTYPE OnStartComposition(ITfCompositionView*, BOOL* accepted) override {
        *accepted = TRUE; ++started; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE OnUpdateComposition(ITfCompositionView*, ITfRange*) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE OnEndComposition(ITfCompositionView* composition) override {
        ComPtr<ITfRange> range;
        auto result = composition->GetRange(&range);
        if (FAILED(result)) return result;
        ComPtr<ITfRangeACP> acp;
        result = range.As(&acp);
        if (FAILED(result)) return result;
        LONG start = 0, length = 0;
        result = acp->GetExtent(&start, &length);
        if (FAILED(result)) return result;
        if (start < 0 || length < 0 || start + length > static_cast<LONG>(text.size())) return TS_E_INVALIDPOS;
        if (length) ended.push_back(text.substr(start, length));
        return S_OK;
    }
};

// All text operations use the real manager and contexts. Only subscription and
// direct keyboard routing are owned, removing dependence on global foreground.
class OwnedManager final : public ITfThreadMgr, public ITfKeystrokeMgr, public ITfSource {
    ULONG references = 1;
    ComPtr<ITfThreadMgr> real;
public:
    ComPtr<ITfKeyEventSink> keys;
    ComPtr<ITfThreadMgrEventSink> events;
    ComPtr<ITfThreadFocusSink> thread_focus;
    bool thread_focused = true;
    unsigned focus_queries = 0;
    explicit OwnedManager(ITfThreadMgr* value) : real(value) {}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** output) override {
        if (!output) return E_POINTER;
        *output = nullptr;
        if (iid == IID_IUnknown || iid == IID_ITfThreadMgr) *output = static_cast<ITfThreadMgr*>(this);
        else if (iid == IID_ITfKeystrokeMgr) *output = static_cast<ITfKeystrokeMgr*>(this);
        else if (iid == IID_ITfSource) *output = static_cast<ITfSource*>(this);
        else if (iid == IID_ITfCompartmentMgr || iid == IID_ITfLangBarItemMgr)
            return real->QueryInterface(iid, output);
        else return E_NOINTERFACE;
        AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++references; }
    ULONG STDMETHODCALLTYPE Release() override {
        const ULONG remaining = --references;
        if (!remaining) delete this;
        return remaining;
    }
    HRESULT STDMETHODCALLTYPE Activate(TfClientId* output) override { return real->Activate(output); }
    HRESULT STDMETHODCALLTYPE Deactivate() override { return real->Deactivate(); }
    HRESULT STDMETHODCALLTYPE CreateDocumentMgr(ITfDocumentMgr** output) override { return real->CreateDocumentMgr(output); }
    HRESULT STDMETHODCALLTYPE EnumDocumentMgrs(IEnumTfDocumentMgrs** output) override { return real->EnumDocumentMgrs(output); }
    HRESULT STDMETHODCALLTYPE GetFocus(ITfDocumentMgr** output) override {
        ++focus_queries;
        return real->GetFocus(output);
    }
    HRESULT STDMETHODCALLTYPE SetFocus(ITfDocumentMgr* value) override {
        ComPtr<ITfDocumentMgr> previous;
        const auto before = real->GetFocus(&previous);
        if (FAILED(before)) return before;
        const auto result = real->SetFocus(value);
        if (FAILED(result)) return result;
        return events ? events->OnSetFocus(value, previous.Get()) : S_OK;
    }
    HRESULT STDMETHODCALLTYPE AssociateFocus(HWND window, ITfDocumentMgr* value, ITfDocumentMgr** previous) override { return real->AssociateFocus(window, value, previous); }
    HRESULT STDMETHODCALLTYPE IsThreadFocus(BOOL* output) override {
        if (!output) return E_POINTER;
        *output = thread_focused ? TRUE : FALSE; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetFunctionProvider(REFCLSID iid, ITfFunctionProvider** output) override { return real->GetFunctionProvider(iid, output); }
    HRESULT STDMETHODCALLTYPE EnumFunctionProviders(IEnumTfFunctionProviders** output) override { return real->EnumFunctionProviders(output); }
    HRESULT STDMETHODCALLTYPE GetGlobalCompartment(ITfCompartmentMgr** output) override { return real->GetGlobalCompartment(output); }
    HRESULT STDMETHODCALLTYPE AdviseSink(REFIID iid, IUnknown* value, DWORD* cookie) override {
        if (!cookie || !value) return E_INVALIDARG;
        if (iid == IID_ITfThreadMgrEventSink) {
            if (events) return CONNECT_E_ADVISELIMIT;
            const auto result = value->QueryInterface(IID_PPV_ARGS(&events));
            if (SUCCEEDED(result)) *cookie = 1;
            return result;
        }
        if (iid == IID_ITfThreadFocusSink) {
            if (thread_focus) return CONNECT_E_ADVISELIMIT;
            const auto result = value->QueryInterface(IID_PPV_ARGS(&thread_focus));
            if (SUCCEEDED(result)) *cookie = 2;
            return result;
        }
        return E_INVALIDARG;
    }
    HRESULT STDMETHODCALLTYPE UnadviseSink(DWORD cookie) override {
        if (cookie == 1 && events) { events.Reset(); return S_OK; }
        if (cookie == 2 && thread_focus) { thread_focus.Reset(); return S_OK; }
        return CONNECT_E_NOCONNECTION;
    }
    HRESULT STDMETHODCALLTYPE AdviseKeyEventSink(TfClientId, ITfKeyEventSink* value, BOOL foreground) override {
        if (!value || !foreground) return E_INVALIDARG;
        if (keys) return CONNECT_E_ADVISELIMIT;
        keys = value; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE UnadviseKeyEventSink(TfClientId) override { keys.Reset(); return S_OK; }
    HRESULT STDMETHODCALLTYPE GetForeground(CLSID*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE TestKeyDown(WPARAM, LPARAM, BOOL*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE TestKeyUp(WPARAM, LPARAM, BOOL*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE KeyDown(WPARAM, LPARAM, BOOL*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE KeyUp(WPARAM, LPARAM, BOOL*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetPreservedKey(ITfContext*, const TF_PRESERVEDKEY*, GUID*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE IsPreservedKey(REFGUID, const TF_PRESERVEDKEY*, BOOL*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE PreserveKey(TfClientId, REFGUID, const TF_PRESERVEDKEY*, const WCHAR*, ULONG) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE UnpreserveKey(REFGUID, const TF_PRESERVEDKEY*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE SetPreservedKeyDescription(REFGUID, const WCHAR*, ULONG) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetPreservedKeyDescription(REFGUID, BSTR*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE SimulatePreservedKey(ITfContext*, REFGUID, BOOL*) override { return E_NOTIMPL; }
};

struct FixtureDocument {
    ComPtr<ITfDocumentMgr> manager;
    ComPtr<FixtureStore> store;
    ComPtr<ITfContext> context;
};

struct ActivationReentry {
    ITfTextInputProcessorEx* processor;
    ITfThreadMgr* manager;
    TfClientId client;
    DWORD flags;
    HHOOK hook = nullptr;
    bool observed = false;
    HRESULT result = E_PENDING;
};
static thread_local ActivationReentry* activation_reentry = nullptr;
static LRESULT CALLBACK owned_activation_hook(int code, WPARAM wparam, LPARAM lparam) {
    auto* fixture = activation_reentry;
    if (code == HCBT_CREATEWND && fixture && !fixture->observed) {
        const auto window = reinterpret_cast<HWND>(wparam);
        DWORD process = 0;
        const auto thread = GetWindowThreadProcessId(window, &process);
        if (thread == GetCurrentThreadId() && process == GetCurrentProcessId()) {
            wchar_t class_name[80]{};
            GetClassNameW(window, class_name, static_cast<int>(sizeof(class_name) / sizeof(*class_name)));
            constexpr wchar_t prefix[] = L"LingoMate.Candidates.";
            if (std::wcsncmp(class_name, prefix, sizeof(prefix) / sizeof(*prefix) - 1) == 0) {
                // Set before the call so even a broken implementation cannot
                // make this test callback recursively activate without bounds.
                fixture->observed = true;
                fixture->result = fixture->processor->ActivateEx(
                    fixture->manager, fixture->client, fixture->flags);
            }
        }
    }
    return CallNextHookEx(fixture ? fixture->hook : nullptr, code, wparam, lparam);
}
class OwnedActivationHook {
    ActivationReentry& fixture;
public:
    explicit OwnedActivationHook(ActivationReentry& value) : fixture(value) {
        require(activation_reentry == nullptr, "Only one owned activation hook may be installed");
        activation_reentry = &fixture;
        // A thread-specific in-process hook: no other application's windows or
        // input are observed, and no global hook or injected module is used.
        fixture.hook = SetWindowsHookExW(WH_CBT, owned_activation_hook, nullptr, GetCurrentThreadId());
        if (!fixture.hook) {
            activation_reentry = nullptr;
            throw std::runtime_error("Create owned thread-only activation hook");
        }
    }
    ~OwnedActivationHook() {
        UnhookWindowsHookEx(fixture.hook);
        activation_reentry = nullptr;
    }
    OwnedActivationHook(const OwnedActivationHook&) = delete;
    OwnedActivationHook& operator=(const OwnedActivationHook&) = delete;
};

class Host {
    bool com_initialized = false;
    bool active = false;
    bool activation_reentry_checked = false;
    HMODULE module = nullptr;
    HMODULE msctf = nullptr;
    HWND window = nullptr;
    BYTE saved_keyboard[256]{};
    ComPtr<ITfThreadMgr> real;
    ComPtr<ITfTextInputProcessorEx> processor;
    ComPtr<OwnedManager> owned;
    std::deque<FixtureDocument> documents;
    TfClientId client = 0;
    DWORD tip_flags = 0;
public:
    ULONGLONG maximum_key_milliseconds = 0;
    Host(const wchar_t* dll, const std::wstring& apartment) {
        require(GetKeyboardState(saved_keyboard) != FALSE, "Save thread keyboard state");
        if (apartment != L"comless") {
            check(CoInitializeEx(nullptr, apartment == L"mta" ? COINIT_MULTITHREADED : COINIT_APARTMENTTHREADED), "COM initialize");
            com_initialized = true;
        }
        window = CreateWindowExW(WS_EX_NOACTIVATE, L"STATIC", L"Owned TSF synthetic document", WS_POPUP,
            -32000, -32000, 640, 480, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
        require(window != nullptr, "Create hidden owned window");
        if (apartment == L"sta") {
            check(CoCreateInstance(CLSID_TF_ThreadMgr, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&real)), "Create real manager");
        } else {
            // Official COM-less factory; retain Msctf.dll until all its objects die.
            msctf = LoadLibraryExW(L"msctf.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
            require(msctf != nullptr, "Load system TSF module");
            using CreateThreadMgr = HRESULT (WINAPI*)(ITfThreadMgr**);
            const auto create = reinterpret_cast<CreateThreadMgr>(GetProcAddress(msctf, "TF_CreateThreadMgr"));
            require(create != nullptr, "Find COM-less manager factory");
            std::cout << "SETUP system COM-less manager factory" << std::endl;
            check(create(&real), "Create COM-less manager");
            tip_flags = TF_TMAE_COMLESS;
        }
        ComPtr<ITfThreadMgrEx> extended;
        check(real.As(&extended), "Extended manager");
        std::cout << "SETUP system TSF ActivateEx before loading fixture DLL" << std::endl;
        check(extended->ActivateEx(&client, TF_TMAE_NOACTIVATETIP | TF_TMAE_NOACTIVATEKEYBOARDLAYOUT | tip_flags), "Activate owned manager without source selection");
        std::cout << "SETUP system TSF activated; setting owned thread compartments" << std::endl;
        owned.Attach(new OwnedManager(real.Get()));
        // These are this fixture process/thread's compartments. Do not inherit
        // the interactive user's current mode or touch global compartments.
        thread_compartment(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, TF_CONVERSIONMODE_NATIVE);
        thread_compartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, 1);
        std::cout << "SETUP loading fixture DLL" << std::endl;
        module = LoadLibraryExW(dll, nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
        require(module != nullptr, "Load fixture TSF DLL");
        std::cout << "SETUP fixture DLL loaded" << std::endl;
        using GetClass = HRESULT (WINAPI*)(REFCLSID, REFIID, void**);
        const auto get_class = reinterpret_cast<GetClass>(GetProcAddress(module, "DllGetClassObject"));
        require(get_class != nullptr, "Find factory export");
        const CLSID clsid{0x078a5202, 0x3f7a, 0x4dd5, {0x89, 0xcc, 0x32, 0x2a, 0x90, 0x11, 0xdf, 0x81}};
        ComPtr<IClassFactory> factory;
        check(get_class(clsid, IID_PPV_ARGS(&factory)), "Factory");
        check(factory->CreateInstance(nullptr, IID_PPV_ARGS(&processor)), "Extended processor");
        std::cout << "SETUP fixture processor created" << std::endl;
    }
    ~Host() {
        if (active) processor->Deactivate();
        try { pump(250); } catch (...) {}
        if (real) real->SetFocus(nullptr);
        for (auto& document : documents) document.manager->Pop(TF_POPF_ALL);
        documents.clear();
        processor.Reset(); owned.Reset();
        if (real) real->Deactivate();
        real.Reset();
        if (window) DestroyWindow(window);
        SetKeyboardState(saved_keyboard);
        if (com_initialized) CoUninitialize();
        if (module) FreeLibrary(module);
        if (msctf) FreeLibrary(msctf);
    }
    FixtureDocument& make_document() {
        FixtureDocument document;
        check(real->CreateDocumentMgr(&document.manager), "Create owned document");
        document.store.Attach(new FixtureStore(window));
        TfEditCookie cookie = 0;
        check(document.manager->CreateContext(client, 0, static_cast<ITextStoreACP*>(document.store.Get()), &document.context, &cookie), "Create real TSF context");
        check(document.manager->Push(document.context.Get()), "Push owned context");
        ComPtr<ITfCompartmentMgr> compartments;
        check(document.context.As(&compartments), "Context compartments");
        for (const GUID* id : {&GUID_COMPARTMENT_KEYBOARD_DISABLED, &GUID_COMPARTMENT_EMPTYCONTEXT}) {
            ComPtr<ITfCompartment> compartment;
            check(compartments->GetCompartment(*id, &compartment), "Owned compartment");
            VARIANT value{}; value.vt = VT_I4; value.lVal = 0;
            check(compartment->SetValue(client, &value), "Enable owned context");
        }
        documents.push_back(std::move(document));
        return documents.back();
    }
    void activate() {
        require(!active, "Fixture already activated");
        const auto previous_focus_queries = owned->focus_queries;
        ActivationReentry reentry{processor.Get(), owned.Get(), client, tip_flags};
        HRESULT result;
        if (!activation_reentry_checked) {
            OwnedActivationHook hook(reentry);
            result = processor->ActivateEx(owned.Get(), client, tip_flags);
        } else {
            result = processor->ActivateEx(owned.Get(), client, tip_flags);
        }
        check(result, "ActivateEx owned processor");
        active = true;
        if (!activation_reentry_checked) {
            require(reentry.observed, "Candidate creation must exercise owned activation reentry");
            require(reentry.result == E_UNEXPECTED,
                "Recursive ActivateEx during owned candidate creation must be rejected");
            activation_reentry_checked = true;
            std::cout << "PASS recursive ActivateEx during owned candidate creation is rejected while outer activation succeeds\n";
        }
        require(owned->keys && owned->events && owned->thread_focus,
            "Activation must advise key, document and thread focus sinks");
        require(owned->focus_queries > previous_focus_queries,
            "Activation must query existing document focus without waiting for a notification");
    }
    void deactivate() {
        check(processor->Deactivate(), "Deactivate owned processor");
        active = false;
        require(!owned->keys && !owned->events && !owned->thread_focus,
            "Deactivation must release key, document and thread focus subscriptions");
    }
    void focus(FixtureDocument& document) { check(owned->SetFocus(document.manager.Get()), "Owned document focus"); }
    void thread_compartment(REFGUID id, LONG value) {
        ComPtr<ITfCompartmentMgr> compartments;
        check(owned.As(&compartments), "Owned thread compartments");
        ComPtr<ITfCompartment> compartment;
        check(compartments->GetCompartment(id, &compartment), "Owned thread mode compartment");
        VARIANT variant{}; variant.vt = VT_I4; variant.lVal = value;
        check(compartment->SetValue(client, &variant), "Set owned thread mode");
    }
    LONG thread_compartment(REFGUID id) {
        ComPtr<ITfCompartmentMgr> compartments;
        check(owned.As(&compartments), "Owned thread compartments");
        ComPtr<ITfCompartment> compartment;
        check(compartments->GetCompartment(id, &compartment), "Read owned thread mode compartment");
        VARIANT value{};
        check(compartment->GetValue(&value), "Read owned thread mode");
        const bool integer = value.vt == VT_I4;
        const LONG result = integer ? value.lVal : 0;
        VariantClear(&value);
        require(integer, "Owned thread mode must have an integer value");
        return result;
    }
    void click_mode(bool english, LONG preserved_conversion_flags) {
        ComPtr<ITfLangBarItemMgr> manager;
        check(owned.As(&manager), "Owned language-bar manager");
        ComPtr<ITfLangBarItem> item;
        check(manager->GetItem(GUID_LBI_INPUTMODE, &item), "Owned input-mode language-bar item");
        TF_LANGBARITEMINFO info{};
        check(item->GetInfo(&info), "Owned input-mode item identity");
        const CLSID clsid{0x078a5202, 0x3f7a, 0x4dd5, {0x89, 0xcc, 0x32, 0x2a, 0x90, 0x11, 0xdf, 0x81}};
        require(info.clsidService == clsid && info.guidItem == GUID_LBI_INPUTMODE,
            "Fixture may click only its own LingoMate input-mode item");
        ComPtr<ITfLangBarItemButton> button;
        check(item.As(&button), "Owned input-mode language-bar button");
        const POINT point{};
        const RECT area{};
        check(button->OnClick(TF_LBI_CLK_LEFT, point, &area), "Synthetic left click on owned input-mode button");
        pump(20);
        const LONG open = thread_compartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
        const LONG conversion = thread_compartment(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION);
        require(open == (english ? 0 : 1), "Language-bar toggle must synchronize system open mode");
        const LONG expected_conversion = preserved_conversion_flags | (english ? 0 : TF_CONVERSIONMODE_NATIVE);
        require(conversion == expected_conversion,
            "Language-bar toggle must change only the native conversion bit");
        BSTR text = nullptr;
        check(button->GetText(&text), "Owned input-mode language-bar text");
        const bool correct_text = text && std::wstring(text, SysStringLen(text)) == (english ? L"英" : L"中");
        SysFreeString(text);
        require(correct_text, "Owned language-bar text must show the actual system mode");
    }
    void thread_focus(bool focused) {
        require(active && owned->thread_focus, "Thread focus fixture requires active subscription");
        owned->thread_focused = focused;
        if (focused) check(owned->thread_focus->OnSetThreadFocus(), "Owned thread gains focus");
        else check(owned->thread_focus->OnKillThreadFocus(), "Owned thread loses focus");
    }
    bool key(FixtureDocument& document, WPARAM code, bool shift = false) {
        BYTE keyboard[256]{};
        if (shift) keyboard[VK_SHIFT] = keyboard[VK_LSHIFT] = 0x80;
        require(SetKeyboardState(keyboard) != FALSE, "Set fixture thread key state");
        const auto begin = GetTickCount64();
        BOOL consumed = FALSE;
        check(owned->keys->OnTestKeyDown(document.context.Get(), code, 0, &consumed), "Test direct synthetic key");
        if (consumed) {
            check(owned->keys->OnKeyDown(document.context.Get(), code, 0, &consumed), "Direct synthetic key");
            require(consumed != FALSE, "Test and delivery disagree on consumed key");
        }
        maximum_key_milliseconds = std::max(maximum_key_milliseconds, GetTickCount64() - begin);
        return consumed != FALSE;
    }
    void type(FixtureDocument& document, const std::string& text) {
        for (unsigned char letter : text) {
            require(std::isalpha(letter) != 0, "Only synthetic alphabet fixture input is supported");
            require(key(document, static_cast<WPARAM>(std::toupper(letter)), std::isupper(letter) != 0), "Synthetic alphabet key must be consumed");
        }
    }
    void expect(FixtureDocument& document, const std::wstring& text, const char* scenario) {
        const auto deadline = GetTickCount64() + 6000;
        while (document.store->text != text && GetTickCount64() < deadline) pump(10);
        // Drain subsequent messages too: an early matching frame must not hide a duplicate commit.
        pump(250);
        require(document.store->text == text, scenario);
        std::cout << "PASS " << scenario << '\n';
    }
    void expect_finished_raw(FixtureDocument& document, const std::wstring& text, const char* scenario) {
        const auto deadline = GetTickCount64() + 6000;
        while ((!document.store->count(text) || owned_candidates()) && GetTickCount64() < deadline)
            pump(10);
        pump(250);
        require(document.store->text == text && document.store->count(text) == 1,
            "Thread focus boundary must finish the original raw composition once");
        require(owned_candidates() == nullptr, "Thread focus loss must hide the candidate window");
        std::cout << "PASS " << scenario << '\n';
    }
    void hover_candidates(FixtureDocument& document) {
        // The fixture's wode result has five paired candidates. The compact
        // layout uses a 560px candidate width and a 400px loading/empty width;
        // both states can have the same height. Width plus the fixture-owned
        // raw anchor therefore distinguishes a real worker reply from loading.
        const auto deadline = GetTickCount64() + 6000;
        HWND candidates = nullptr;
        RECT client_rect{};
        unsigned dpi = 96;
        int row = 44;
        int padding = 12;
        int inset = 8;
        int body_separator = 9;
        bool ready = false;
        while (GetTickCount64() < deadline) {
            pump(10);
            candidates = owned_candidates();
            if (!candidates || document.store->text != L"wode") continue;
            // The candidate window keeps its own per-monitor DPI context. Measure it
            // in that context so its client size matches GetDpiForWindow.
            const auto fixture_context = SetThreadDpiAwarenessContext(GetWindowDpiAwarenessContext(candidates));
            require(fixture_context != nullptr, "Enter own candidate DPI context");
            const bool measured = GetClientRect(candidates, &client_rect) != FALSE;
            SetThreadDpiAwarenessContext(fixture_context);
            require(measured, "Read own candidate client rectangle");
            dpi = std::max(96u, GetDpiForWindow(candidates));
            row = static_cast<int>((44 * dpi + 48) / 96);
            padding = static_cast<int>((12 * dpi + 48) / 96);
            inset = static_cast<int>((8 * dpi + 48) / 96);
            body_separator = 2 * static_cast<int>((4 * dpi + 48) / 96)
                + static_cast<int>((dpi + 48) / 96);
            const auto loaded_width = static_cast<int>((560 * dpi + 48) / 96);
            // Allow only the native border's rounding tolerance. A 400px
            // loading frame must never satisfy this candidate-ready predicate.
            ready = client_rect.right >= loaded_width - 2 && client_rect.right <= loaded_width + 2;
            if (ready) break;
        }
        require(ready, "Real wode candidates must be ready before hover stress");
        const int candidate_x = padding + inset;
        const int chinese_y = padding + row + body_separator + inset;
        const int english_y = chinese_y + row;
        require(candidate_x < client_rect.right - padding
            && english_y < client_rect.bottom - padding,
            "Synthetic hover points must be inside the first paired candidate rows");
        require(document.store->count(L"我的") == 0, "Hover test must start before candidate commit");
        RECT original{};
        require(GetWindowRect(candidates, &original) != FALSE, "Save own candidate rectangle");
        // Move only this test-owned popup outside the screen. The real pointer is
        // never read or moved; TrackMouseEvent sees a deterministic outside point.
        require(SetWindowPos(candidates, nullptr, -32000, -32000, 0, 0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER) != FALSE, "Place own hover fixture offscreen");
        pump(20);
        const auto started = GetTickCount64();
        unsigned leaves = 0;
        for (unsigned cycle = 0; cycle < 64; ++cycle) {
            SendMessageW(candidates, WM_MOUSEMOVE, 0, MAKELPARAM(candidate_x, chinese_y));
            SendMessageW(candidates, WM_MOUSEMOVE, 0, MAKELPARAM(candidate_x, english_y));
            SendMessageW(candidates, WM_MOUSELEAVE, 0, 0);
            const auto generated = pump(6, candidates);
            require(generated <= 1, "Mouse leave must settle after one queued event per reentry");
            leaves += generated;
            require(GetTickCount64() - started < 5000, "Hover stress exceeds bounded responsiveness deadline");
        }
        SendMessageW(candidates, WM_MOUSELEAVE, 0, 0);
        require(pump(30, candidates) == 0, "An outside mouse leave must never rearm tracking");
        require(document.store->text == L"wode" && document.store->count(L"我的") == 0,
            "Hover and leave must neither commit nor alter the owned composition");
        require(SetWindowPos(candidates, nullptr, original.left, original.top, 0, 0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER) != FALSE, "Restore own candidate rectangle");
        std::cout << "PASS candidate hover, leave and reentry stress cycles=64 queued_leaves=" << leaves
            << " elapsed_ms=" << GetTickCount64() - started << '\n';
    }
    void restart() { deactivate(); pump(250); activate(); }
};

static void scenarios(Host& host) {
    // No pumping between keys: includes a second Return while earlier work is pending.
    auto& mixed = host.make_document(); host.focus(mixed); host.activate();
    host.type(mixed, "wo"); require(host.key(mixed, VK_SPACE), "Queued Space");
    host.type(mixed, "AI"); require(host.key(mixed, VK_RETURN), "Queued Return");
    host.key(mixed, VK_RETURN);
    host.type(mixed, "xuexi"); require(host.key(mixed, VK_SPACE), "Queued second Space");
    host.expect(mixed, L"我AI学习", "rapid mixed FIFO and double Return");
    require(mixed.store->count(L"我") == 1 && mixed.store->count(L"AI") == 1 && mixed.store->count(L"学习") == 1,
        "Each mixed composition must commit once");
    std::cout << "PASS already-focused context is usable immediately after activation without a focus notification\n";
    require(!host.key(mixed, VK_RETURN), "Drained empty Return must be left to host");
    host.expect(mixed, L"我AI学习", "empty Return does not duplicate output");

    host.restart();
    auto& word = host.make_document(); host.focus(word);
    host.type(word, "wode"); require(host.key(word, VK_SPACE), "Queued wode Space");
    host.expect(word, L"我的", "same COM object reactivates with a fresh worker");
    require(word.store->count(L"我的") == 1, "wode composition must commit once");

    host.restart();
    auto& modes = host.make_document(); host.focus(modes);
    host.thread_compartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, 0);
    pump(20);
    require(!host.key(modes, 'W'), "System closed mode must leave alphabet keys to the host");
    require(modes.store->text.empty(), "Closed mode must not create a hidden composition");
    host.thread_compartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, 1);
    pump(20);
    host.type(modes, "wo"); require(host.key(modes, VK_SPACE), "Space after system open mode restoration");
    host.expect(modes, L"我", "system open mode switches off and restores Chinese input");
    host.thread_compartment(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, 0);
    pump(20);
    require(!host.key(modes, 'W'), "System alphanumeric conversion must leave alphabet keys to the host");
    host.thread_compartment(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, TF_CONVERSIONMODE_NATIVE);
    host.thread_compartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, 1);
    pump(20);
    host.type(modes, "wo"); require(host.key(modes, VK_SPACE), "Space after native conversion restoration");
    host.expect(modes, L"我我", "system conversion mode switches off and restores Chinese input");
    require(modes.store->count(L"我") == 2, "Mode switches must not replay earlier commits");
    constexpr LONG preserved_conversion = TF_CONVERSIONMODE_FULLSHAPE | TF_CONVERSIONMODE_SYMBOL;
    host.thread_compartment(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,
        preserved_conversion | TF_CONVERSIONMODE_NATIVE);
    host.click_mode(true, preserved_conversion);
    require(!host.key(modes, 'W'), "Language-bar English mode must leave alphabet keys to the host");
    host.click_mode(false, preserved_conversion);
    host.type(modes, "wo"); require(host.key(modes, VK_SPACE), "Space after language-bar Chinese restoration");
    host.expect(modes, L"我我我", "language-bar left click synchronizes compartments and restores Chinese input");
    require(modes.store->count(L"我") == 3, "Language-bar toggles must not replay earlier commits");

    host.restart();
    auto& threaded = host.make_document(); host.focus(threaded);
    host.type(threaded, "wode");
    // There is no document-focus or key-focus notification between these calls.
    // Even an in-flight reply must finish only in its originating store.
    host.thread_focus(false);
    require(!host.key(threaded, 'W'), "Unfocused thread must leave alphabet keys to the host");
    host.expect_finished_raw(threaded, L"wode", "thread focus loss finishes pending raw input once and hides candidates");
    host.thread_focus(true);
    host.type(threaded, "wo"); require(host.key(threaded, VK_SPACE), "Space after same-document thread focus restoration");
    host.expect(threaded, L"wode我", "same document resumes after thread focus restoration without replay");
    require(threaded.store->count(L"wode") == 1 && threaded.store->count(L"我") == 1
        && threaded.store->count(L"我的") == 0,
        "Thread focus restoration must preserve raw once and never replay a retired candidate");

    host.restart();
    auto& hovered = host.make_document(); host.focus(hovered);
    host.type(hovered, "wode");
    host.hover_candidates(hovered);
    require(host.key(hovered, VK_SPACE), "Space remains responsive after candidate hover stress");
    host.expect(hovered, L"我的", "Space commits correct candidate once after mouse stress");
    require(hovered.store->count(L"我的") == 1, "Hover stress must preserve exactly one candidate commit");

    host.restart();
    auto& old_document = host.make_document(); host.focus(old_document);
    host.type(old_document, "wode");
    // Keep the originating store for explicit cross-context assertions.
    const auto old_store = old_document.store;
    auto& new_document = host.make_document(); host.focus(new_document);
    host.type(new_document, "wo"); require(host.key(new_document, VK_SPACE), "New context Space");
    host.expect(new_document, L"我", "old query never writes into the new context");
    require(old_store->text == L"wode" && old_store->count(L"wode") == 1,
        "Focus boundary must preserve old raw input once in its original context");

    host.restart();
    auto& terminated = host.make_document(); host.focus(terminated);
    host.type(terminated, "wode");
    const auto deadline = GetTickCount64() + 6000;
    while ((!terminated.store->started || terminated.store->text != L"wode") && GetTickCount64() < deadline) pump(10);
    require(terminated.store->started && terminated.store->text == L"wode", "Owned composition anchor must exist before termination");
    require(host.key(terminated, VK_SPACE), "Queued commit before owner termination");
    ComPtr<ITfContextOwnerCompositionServices> owner_services;
    check(terminated.context.As(&owner_services), "Owner composition services");
    check(owner_services->TerminateComposition(nullptr), "Owner terminates own composition");
    const auto retained = terminated.store->text;
    host.focus(terminated);
    host.type(terminated, "xuexi"); require(host.key(terminated, VK_SPACE), "Reattached context Space");
    host.expect(terminated, retained + L"学习", "terminated generation cannot commit after same-context reattach");
    require(terminated.store->count(L"我的") == 0, "Terminated candidate must never commit");

    host.restart();
    auto& cold = host.make_document(); host.focus(cold);
    host.type(cold, "WoDe");
    host.deactivate();
    const auto cold_deadline = GetTickCount64() + 6000;
    while (cold.store->count(L"WoDe") == 0 && GetTickCount64() < cold_deadline) pump(10);
    pump(250);
    require(cold.store->text == L"WoDe" && cold.store->count(L"WoDe") == 1,
        "Deactivation must flush queued raw input once, never a candidate");
    std::cout << "PASS deactivate during pending initialization flushes raw once\n";
    host.activate();
    auto& reactivated = host.make_document(); host.focus(reactivated);
    host.type(reactivated, "wo"); require(host.key(reactivated, VK_SPACE), "Reactivated Space");
    host.expect(reactivated, L"我", "reactivation after cold deactivation remains usable");
    require(reactivated.store->count(L"我") == 1 && cold.store->count(L"WoDe") == 1,
        "Reactivation must not replay retired commits");
    std::cout << "maximum_direct_key_callback_ms=" << host.maximum_key_milliseconds << '\n';
}

int wmain(int argc, wchar_t** argv) {
    if (argc != 4) return 2;
    const std::wstring apartment = argv[3];
    if (apartment != L"sta" && apartment != L"mta" && apartment != L"comless") return 2;
    require(SetEnvironmentVariableW(L"LOCALAPPDATA", argv[1]) != FALSE, "Isolate local data directory");
    const auto test_session=L"tsf-"+std::to_wstring(GetCurrentProcessId());
    require(SetEnvironmentVariableW(L"LINGOMATE_TEST_SESSION",test_session.c_str()) != FALSE,"Isolate settings session");
    try {
        Host host(argv[2], apartment);
        std::cout << "SETUP running owned scenarios" << std::endl;
        scenarios(host);
        std::cout << "PASS owned TSF contexts, direct sinks and ActivateEx; system loading and real apps are outside this fixture\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "FAIL " << error.what() << '\n';
        return 1;
    }
}
