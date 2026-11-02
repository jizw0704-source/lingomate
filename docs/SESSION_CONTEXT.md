# Mac 当前会话上下文排序

## 行为

只在用户继续输入拼音后，利用灵果刚刚确认输出的中文尾部调整同音词排序。空白输入不显示接续词，不生成额外候选，不读取聊天正文、剪贴板、其他应用内容或历史输入文件。

首版使用有界本地搭配规则。例如先确认“临床”，再输入 shiyan，“试验”可前移；先确认“客观”，再输入 shishi，“事实”可前移。它是轻量规则评分，不是通用语义模型，未覆盖的表达保持原排序。

只对完整输入精确匹配、相同读音/消费长度组内的既有词加分；简拼、未完整音节、分段候选、表情符号和整句补全不由上下文强行前移。显式优先/减少反馈及个人词库手动优先仍高于上下文分数。次数、读音、词库格式和提交验证流程不变。

## 会话和隐私

- Native ConfirmedContext 只在成功调用宿主 insertText 输出中文后更新，最多保留尾部32个汉字；超过60秒未确认中文后不再参与排序。
- 非中文输出、原样回车、符号/标点、删除、光标/鼠标边界、取消输入、输入框/客户端更换、输入源失活和英文切换清空缓存。空的宿主 commitComposition 回调保留当前会话缓存；它不代表新输出。
- 查询经本机匿名管道携带可选 preceding 字段，不进入响应、诊断、账户同步或翻译服务。桥接只在 Mac 专用路径使用；每次 query 替换当前缓存，缺省/空输入清空，非同 scope commit 及 cancel/confirm 等操作清空。
- 引擎标识变化丢弃旧上下文并只重新查询；不重放提交或确认。上下文不写入文件，不记录完整输入轨迹。原有对已确认词语的个人词库记忆继续按原规则保存。
- 测试只使用独立编写的合成文本和临时词库。没有新增外部语料、云端模型或生产依赖，新增代码与规则按项目GPL-3.0-or-later提供；原词库署名继续随包保留。

## 评测和限制

原60个无上下文工程用例保持54首选、58前五/召回，没有逐条排名退步。新增 context-cases.json 的12个规则工程示例，旧版首选6/12，本版9/12，召回均12/12；它们不是盲测或真实用户统计，不能据此宣称普遍准确率提升。部分例子（如提出异议、测量电导）仍不一定升到首选，保留失败项。

```sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
runtime/python/bin/python prototype/tests/session_context_test.py
native-macos/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --confirmed-context-test
runtime/python/bin/python native-macos/tools/evaluate_candidates.py --bridge upstream/qingjian/target/release/bilingual-ime-bridge --cases native-macos/evaluation/context-cases.json --output evidence/context/current.json
```

Windows/legacy默认排序不启用此字段，已做本地默认路径对照；Windows真实构建和实机不属于本次验收。宿主真实键盘、跨应用、长期使用及休眠唤醒分别验收，自动检查不代替实体键盘反馈。
