// Explicit installed-profile integration only, never a release binary, default
// CI check or real keyboard acceptance test. Run via test_system_profile.ps1
// only within the user's authorized local native testing scope.
// This program operates only its own STA thread, empty ACP document and owned
// window. --own-focus briefly displays/activates that window and restores the
// previous foreground HWND without reading another application's text/title.
// Hidden mode reports deferred loading; it cannot accept a focused profile
// switch. Ordinary Activate and ActivateEx(TF_TMAE_COMLESS) independently test
// Windows' installed-profile loading paths. This is COM-less loader selection,
// not an assertion that the caller's STA was never initialized. It uses the
// installed profile through the real Windows TSF manager;
// it never loads the LingoMate DLL manually, invokes its factory/direct sinks,
// registers/enables a profile, changes a default/session/process-wide profile,
// bypasses a denied foreground request, dispatches keys, reads external text or
// accesses the clipboard. Its output contains only technical phases, HRESULTs,
// CLSIDs and interface IDs. The copied ACP fixture contains no user text.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <msctf.h>
#include <textstor.h>
#include <olectl.h>
#include <wrl/client.h>
#include <algorithm>
#include <cwchar>
#include <cwctype>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
using Microsoft::WRL::ComPtr;

static void record(const char* phase, HRESULT hr) {
    std::cout << "phase=" << phase << " HRESULT=0x" << std::hex
        << std::uppercase << std::setfill('0') << std::setw(8)
        << static_cast<unsigned long>(hr) << std::dec << '\n';
}
static void record_guid(const char* phase, const char* kind, REFGUID id) {
    wchar_t value[40]{};
    if (!StringFromGUID2(id, value, static_cast<int>(sizeof(value) / sizeof(*value)))) {
        record(phase, E_FAIL); return;
    }
    std::string printable;
    for (const wchar_t* next = value; *next; ++next) printable.push_back(static_cast<char>(*next));
    std::cout << "phase=" << phase << ' ' << kind << '=' << printable << '\n';
}
static void check(HRESULT hr, const char* phase) {
    record(phase, hr);
    if (FAILED(hr)) throw std::runtime_error(phase);
}
static void require(bool condition, const char* phase) {
    record(phase, condition ? S_OK : E_FAIL);
    if (!condition) throw std::runtime_error(phase);
}
static void pump(DWORD milliseconds) {
    const auto deadline = GetTickCount64() + milliseconds;
    unsigned processed = 0;
    do {
        MSG message{};
        unsigned round = 0;
        const auto started = GetTickCount64();
        while (round < 256 && GetTickCount64() - started < 8
            && PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            ++round;
            if (++processed > 8192) {
                record("message_budget", E_ABORT);
                throw std::runtime_error("message_budget");
            }
            if (message.message >= WM_KEYFIRST && message.message <= WM_KEYLAST) {
                record("unexpected_physical_input", E_ABORT);
                throw std::runtime_error("unexpected_physical_input");
            }
            if (message.message != WM_QUIT) DispatchMessageW(&message);
        }
        if (GetTickCount64() < deadline) Sleep(2);
    } while (GetTickCount64() < deadline);
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

static constexpr CLSID lingo_class{0x078a5202, 0x3f7a, 0x4dd5,
    {0x89, 0xcc, 0x32, 0x2a, 0x90, 0x11, 0xdf, 0x81}};
static constexpr GUID lingo_profile{0x5c46b182, 0x7e32, 0x4b6e,
    {0xab, 0x5d, 0x2e, 0xe4, 0xf1, 0x7d, 0x76, 0x95}};
static constexpr LANGID lingo_language = 0x0804;

static bool same_profile(const TF_INPUTPROCESSORPROFILE& a, const TF_INPUTPROCESSORPROFILE& b) {
    return a.dwProfileType == b.dwProfileType && a.langid == b.langid
        && a.clsid == b.clsid && a.guidProfile == b.guidProfile && a.hkl == b.hkl;
}
static std::wstring canonical_path(const std::wstring& path) {
    wchar_t value[32768]{};
    const auto length = GetFullPathNameW(path.c_str(), static_cast<DWORD>(sizeof(value) / sizeof(*value)), value, nullptr);
    require(length && length < sizeof(value) / sizeof(*value), "module_path_canonicalization");
    std::wstring result(value, length);
    std::transform(result.begin(), result.end(), result.begin(), [](wchar_t ch) {
        return ch == L'/' ? L'\\' : static_cast<wchar_t>(std::towlower(ch));
    });
    return result;
}

class Probe {
    bool com_initialized = false;
    bool activated = false;
    bool snapshot_valid = false;
    bool restore_armed = false;
    bool own_focus = false;
    bool focus_ready = false;
    bool associated = false;
    bool foreground_changed = false;
    HWND original_foreground = nullptr;
    HWND window = nullptr;
    TfClientId client = 0;
    ComPtr<ITfThreadMgr> thread;
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    ComPtr<ITfInputProcessorProfiles> languages;
    ComPtr<ITfDocumentMgr> document;
    ComPtr<FixtureStore> store;
    ComPtr<ITfContext> context;
    ComPtr<ITfDocumentMgr> previous_association;
    TF_INPUTPROCESSORPROFILE original{};
public:
    bool restoration_succeeded = true;
    Probe() = default;
    ~Probe() { cleanup(); }
    void cleanup() noexcept {
        if (restore_armed) {
            restore_armed = false;
            // flags=0: no ENABLEPROFILE, FORPROCESS, FORSESSION, or deferred
            // language flag. Restore only the profile saved on this thread.
            const bool processor = original.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR;
            const auto restored = profiles->ActivateProfile(original.dwProfileType,
                original.langid, processor ? original.clsid : CLSID_NULL,
                processor ? original.guidProfile : GUID_NULL, processor ? nullptr : original.hkl, 0);
            record("restore_original_thread_profile", restored);
            TF_INPUTPROCESSORPROFILE current{};
            const auto observed = profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD, &current);
            record("restored_thread_profile_query", observed);
            restoration_succeeded = restored == S_OK && observed == S_OK && same_profile(original, current);
            record("restored_thread_profile_identity", restoration_succeeded ? S_OK : E_FAIL);
        }
        if (associated && thread && window) {
            associated = false;
            ComPtr<ITfDocumentMgr> removed;
            const auto restored = thread->AssociateFocus(window, previous_association.Get(), &removed);
            record("restore_owned_window_document_association", restored);
            restoration_succeeded = restoration_succeeded && restored == S_OK;
        }
        previous_association.Reset();
        if (own_focus && foreground_changed && GetForegroundWindow() == window) {
            if (original_foreground && IsWindow(original_foreground)) {
                const bool restored = SetForegroundWindow(original_foreground) != FALSE
                    && GetForegroundWindow() == original_foreground;
                record("restore_previous_foreground_HWND_only", restored ? S_OK : S_FALSE);
                restoration_succeeded = restoration_succeeded && restored;
            } else {
                record("previous_foreground_HWND_no_longer_available", S_FALSE);
            }
        } else if (own_focus) {
            // Do not override a human focus change made while this test ran.
            record("previous_foreground_restoration_not_needed", S_FALSE);
        }
        if (document) {
            const auto popped = document->Pop(TF_POPF_ALL);
            record("pop_owned_document", popped);
        }
        context.Reset(); store.Reset(); document.Reset();
        if (activated && thread) {
            activated = false;
            record("deactivate_owned_thread_manager", thread->Deactivate());
            try { pump(250); } catch (...) { record("cleanup_message_pump", E_FAIL); }
        }
        thread.Reset(); languages.Reset(); profiles.Reset();
        if (window) { DestroyWindow(window); window = nullptr; }
        if (com_initialized) { CoUninitialize(); com_initialized = false; }
    }
    bool focused() const { return focus_ready; }
    void initialize(bool comless, bool requested_focus) {
        own_focus = requested_focus;
        if (own_focus) original_foreground = GetForegroundWindow(); // Handle only.
        const auto initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
        check(initialized, "initialize_owned_STA");
        com_initialized = true;
        window = CreateWindowExW(own_focus ? 0 : WS_EX_NOACTIVATE, L"STATIC",
            own_focus ? L"LingoMate loading check - no typing required" : L"",
            own_focus ? WS_OVERLAPPEDWINDOW : WS_POPUP,
            own_focus ? CW_USEDEFAULT : -32000, own_focus ? CW_USEDEFAULT : -32000,
            own_focus ? 420 : 640, own_focus ? 140 : 480,
            nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
        require(window != nullptr, "create_owned_probe_window");
        check(CoCreateInstance(CLSID_TF_ThreadMgr, nullptr, CLSCTX_INPROC_SERVER,
            IID_PPV_ARGS(&thread)), "create_real_thread_manager");
        record_guid("real_thread_manager_interface", "IID", IID_ITfThreadMgr);
        // Ordinary and forced COM-less loader activation are intentional.
        // There is no OwnedManager shim or NOACTIVATETIP /
        // NOACTIVATEKEYBOARDLAYOUT suppression of OS loading.
        if (comless) {
            ComPtr<ITfThreadMgrEx> extended;
            check(thread.As(&extended), "extended_thread_manager_interface");
            check(extended->ActivateEx(&client, TF_TMAE_COMLESS), "COMLESS_thread_manager_activate");
        } else {
            check(thread->Activate(&client), "ordinary_thread_manager_activate");
        }
        activated = true;
        check(CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr,
            CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&profiles)), "create_real_profile_manager");
        record_guid("real_profile_manager_interface", "IID", IID_ITfInputProcessorProfileMgr);
        check(profiles.As(&languages), "profile_language_interface");
        const auto saved = profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD, &original);
        check(saved, "save_original_thread_profile");
        require(saved == S_OK, "original_thread_profile_available");
        require(original.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR
            || original.dwProfileType == TF_PROFILETYPE_KEYBOARDLAYOUT,
            "original_thread_profile_type_supported");
        snapshot_valid = true;
        record_guid("original_thread_profile_service", "CLSID", original.clsid);

        check(thread->CreateDocumentMgr(&document), "create_owned_document_manager");
        store.Attach(new FixtureStore(window));
        TfEditCookie cookie = 0;
        check(document->CreateContext(client, 0, static_cast<ITextStoreACP*>(store.Get()),
            &context, &cookie), "create_owned_empty_ACP_context");
        check(document->Push(context.Get()), "push_owned_context");
        ComPtr<ITfCompartmentMgr> compartments;
        check(context.As(&compartments), "owned_context_compartment_interface");
        for (const GUID* id : {&GUID_COMPARTMENT_KEYBOARD_DISABLED, &GUID_COMPARTMENT_EMPTYCONTEXT}) {
            ComPtr<ITfCompartment> compartment;
            check(compartments->GetCompartment(*id, &compartment), "owned_context_compartment");
            VARIANT value{}; value.vt = VT_I4; value.lVal = 0;
            check(compartment->SetValue(client, &value), "enable_owned_empty_context");
        }
        check(thread->SetFocus(document.Get()), "set_owned_TSF_document_focus");
        if (own_focus) {
            check(thread->AssociateFocus(window, document.Get(), &previous_association),
                "associate_owned_window_with_empty_ACP_document");
            associated = true;
            ShowWindow(window, SW_SHOWNOACTIVATE);
            UpdateWindow(window);
            const bool accepted = SetForegroundWindow(window) != FALSE;
            record("own_foreground_request_accepted", accepted ? S_OK : S_FALSE);
            if (accepted && GetForegroundWindow() == window) {
                foreground_changed = true;
                SetFocus(window);
            }
            pump(400);
            const bool native_focus = accepted && GetForegroundWindow() == window && GetFocus() == window;
            record("owned_native_window_has_actual_focus", native_focus ? S_OK : S_FALSE);
            BOOL thread_focus = FALSE;
            const auto queried = thread->IsThreadFocus(&thread_focus);
            record("owned_focused_thread_query", queried);
            focus_ready = native_focus && queried == S_OK && thread_focus;
            record("owned_TSF_thread_has_actual_focus", focus_ready ? S_OK : S_FALSE);
            if (!focus_ready)
                record("own_focus_refused_no_bypass_loading_check_deferred", S_FALSE);
        }
        BOOL focused = FALSE;
        check(thread->IsThreadFocus(&focused), "owned_thread_focus_query");
        record("owned_thread_has_input_focus", focused ? S_OK : S_FALSE);
    }
    bool inspect_module(const std::wstring& expected, const char* presence_phase, bool mandatory) {
        // GetModuleHandle does not load the DLL. A fresh probe process can only
        // have this module because Windows activated the registered text service.
        const auto module = GetModuleHandleW(expected.c_str());
        record(presence_phase, module ? S_OK : S_FALSE);
        if (!module) {
            if (mandatory) throw std::runtime_error(presence_phase);
            return false;
        }
        wchar_t loaded[32768]{};
        const auto length = GetModuleFileNameW(module, loaded, static_cast<DWORD>(sizeof(loaded) / sizeof(*loaded)));
        require(length && length < sizeof(loaded) / sizeof(*loaded), "loaded_module_path_metadata");
        require(canonical_path(std::wstring(loaded, length)) == canonical_path(expected),
            "loaded_module_matches_expected_installation");
        const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(module);
        require(dos->e_magic == IMAGE_DOS_SIGNATURE && dos->e_lfanew > 0,
            "loaded_module_DOS_metadata");
        const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS*>(
            reinterpret_cast<const unsigned char*>(module) + dos->e_lfanew);
        require(nt->Signature == IMAGE_NT_SIGNATURE
            && nt->FileHeader.Machine == IMAGE_FILE_MACHINE_AMD64
            && nt->OptionalHeader.Magic == IMAGE_NT_OPTIONAL_HDR64_MAGIC,
            "loaded_module_x64_PE_metadata");
        return true;
    }
    void observe_thread_mode(bool after) {
        const std::string suffix = after ? "_after_activation" : "_before_activation";
        ComPtr<ITfCompartmentMgr> compartments;
        const auto queried = thread.As(&compartments);
        record(("owned_thread_mode_interface" + suffix).c_str(), queried);
        if (SUCCEEDED(queried)) {
            for (const GUID* id : {&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
                &GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION}) {
                const bool open = *id == GUID_COMPARTMENT_KEYBOARD_OPENCLOSE;
                const std::string name = open ? "owned_thread_open" : "owned_thread_conversion";
                ComPtr<ITfCompartment> compartment;
                const auto located = compartments->GetCompartment(*id, &compartment);
                record((name + "_compartment" + suffix).c_str(), located);
                if (FAILED(located)) continue;
                VARIANT value{};
                const auto obtained = compartment->GetValue(&value);
                record((name + "_value" + suffix).c_str(), obtained);
                const bool integer = SUCCEEDED(obtained) && value.vt == VT_I4;
                record((name + "_value_is_VT_I4" + suffix).c_str(), integer ? S_OK : S_FALSE);
                if (integer) {
                    if (open) {
                        record(("owned_thread_mode_is_open" + suffix).c_str(), value.lVal ? S_OK : S_FALSE);
                    } else {
                        const bool non_native = (static_cast<DWORD>(value.lVal) & TF_CONVERSIONMODE_NATIVE) == 0;
                        record(("owned_thread_mode_is_non_native" + suffix).c_str(), non_native ? S_OK : S_FALSE);
                    }
                }
                VariantClear(&value);
            }
        }
        BOOL focused = FALSE;
        const auto observed = thread->IsThreadFocus(&focused);
        record(("owned_thread_focus_query" + suffix).c_str(), observed);
        if (SUCCEEDED(observed))
            record(("owned_thread_has_input_focus" + suffix).c_str(), focused ? S_OK : S_FALSE);
    }
    int investigate(const std::wstring& expected_module) {
        require(snapshot_valid, "original_thread_profile_saved_before_probe");
        LANGID current_language = 0;
        check(languages->GetCurrentLanguage(&current_language), "current_thread_language_query");
        // DONTCARECURRENTINPUTLANGUAGE would only queue future activation, not
        // make a successful immediate load. Refuse a mismatch instead.
        require(current_language == lingo_language, "current_thread_language_matches_LingoMate");
        observe_thread_mode(false);
        inspect_module(expected_module, "owned_module_present_before_profile_activation", false);
        restore_armed = true; // Restore even if the next call fails partially.
        const auto activation = profiles->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,
            lingo_language, lingo_class, lingo_profile, nullptr, 0);
        check(activation, "activate_LingoMate_profile_current_thread_flags_zero");
        // S_FALSE means disabled. SUCCEEDED alone is insufficient.
        require(activation == S_OK, "profile_activation_was_S_OK");
        pump(400);
        observe_thread_mode(true);
        if (own_focus) {
            BOOL focused = FALSE;
            const auto queried = thread->IsThreadFocus(&focused);
            record("owned_TSF_thread_focus_after_profile_switch_query", queried);
            const bool retained = queried == S_OK && focused
                && GetForegroundWindow() == window && GetFocus() == window;
            record("owned_TSF_thread_still_has_actual_focus", retained ? S_OK : S_FALSE);
            if (!retained) {
                record("own_focus_lost_loading_check_deferred", S_FALSE);
                return 3;
            }
        }
        TF_INPUTPROCESSORPROFILE active{};
        const auto observed = profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD, &active);
        check(observed, "active_thread_profile_query");
        require(observed == S_OK, "active_thread_profile_available");
        record_guid("active_thread_profile_service", "CLSID", active.clsid);
        require(active.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR
            && active.langid == lingo_language && active.clsid == lingo_class
            && active.guidProfile == lingo_profile,
            "active_thread_profile_matches_LingoMate");
        ComPtr<ITfKeystrokeMgr> keys;
        check(thread.As(&keys), "real_keystroke_manager_interface");
        record_guid("real_keystroke_manager_interface", "IID", IID_ITfKeystrokeMgr);
        CLSID foreground{};
        const auto foreground_result = keys->GetForeground(&foreground);
        record("foreground_text_service_query", foreground_result);
        if (foreground_result == S_OK) {
            record_guid("foreground_text_service", "CLSID", foreground);
            if (own_focus) require(foreground == lingo_class, "foreground_text_service_matches_LingoMate");
            else record("background_foreground_text_service_matches_LingoMate",
                foreground == lingo_class ? S_OK : S_FALSE);
        }
        ComPtr<ITfDocumentMgr> focused_document;
        const auto focus_result = thread->GetFocus(&focused_document);
        record("focused_owned_document_query", focus_result);
        require(focus_result == S_OK && focused_document.Get() == document.Get(),
            "focused_document_matches_fixture");
        if (!own_focus) {
            // A hidden/background thread can change active-profile metadata
            // while Windows waits for real focus to activate the newly chosen
            // TIP. S_OK from ActivateProfile alone never establishes loading.
            inspect_module(expected_module, "module_observed_in_background_thread", false);
            require(store->text.empty() && store->ended.empty(), "owned_synthetic_document_remains_empty");
            record("hidden_profile_switch_loading_deferred_without_focused_acceptance", S_FALSE);
            return 3;
        }
        require(foreground_result == S_OK, "focused_foreground_text_service_available");
        inspect_module(expected_module, "owned_module_loaded_by_Windows_profile_activation", true);
        require(store->text.empty() && store->ended.empty(), "owned_synthetic_document_remains_empty");
        record("system_loading_probe_only_not_keyboard_acceptance", S_OK);
        return 0;
    }
};

int wmain(int argc, wchar_t** argv) {
    // argv[1]: trusted expected installed DLL; argv[2]: new probe-owned local
    // data directory. Never print these paths or use a real vocabulary/config.
    if (argc < 3 || argc > 5) { record("arguments", E_INVALIDARG); return 2; }
    bool comless = false, own_focus = false;
    for (int index = 3; index < argc; ++index) {
        if (wcscmp(argv[index], L"--comless") == 0 && !comless) comless = true;
        else if (wcscmp(argv[index], L"--own-focus") == 0 && !own_focus) own_focus = true;
        else { record("arguments", E_INVALIDARG); return 2; }
    }
    if (!CreateDirectoryW(argv[2], nullptr)) {
        record("create_fresh_probe_local_data", HRESULT_FROM_WIN32(GetLastError())); return 2;
    }
    record("create_fresh_probe_local_data", S_OK);
    const auto test_session=L"profile-"+std::to_wstring(GetCurrentProcessId());
    if (!SetEnvironmentVariableW(L"LINGOMATE_TEST_SESSION",test_session.c_str()) || !SetEnvironmentVariableW(L"LOCALAPPDATA", argv[2])) {
        record("isolate_probe_local_data", HRESULT_FROM_WIN32(GetLastError())); return 2;
    }
    record("isolate_probe_local_data", S_OK);
    Probe probe;
    int result = 0;
    try {
        probe.initialize(comless, own_focus);
        if (own_focus && !probe.focused()) {
            record("focused_system_loading_check_not_established", S_FALSE);
            result = 3;
        } else {
            result = probe.investigate(argv[1]);
        }
    } catch (...) {
        record("probe_incomplete_or_failed", E_FAIL);
        result = 1;
    }
    probe.cleanup();
    if (!probe.restoration_succeeded) result = 1;
    record("probe_process_exit", result == 0 ? S_OK : (result == 3 ? S_FALSE : E_FAIL));
    return result;
}

