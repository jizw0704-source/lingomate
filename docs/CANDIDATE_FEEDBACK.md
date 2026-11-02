# Mac 候选推荐反馈

入口：候选窗右侧“⋯”→“调整词语推荐”→选择词语→优先推荐／减少推荐／恢复自动排序。

仅完整读音精确匹配的基础词或已确认个人词可操作。简拼、部分消费、整句合成、固定表达补全、英文原样输入、表情和符号不进入此菜单。保持同读音及消费长度组内排序；减少推荐不删除词条，仍可翻页选择。一个读音只保留一个显式优先词；不上传、不收集正文、剪贴板或输入过程。

显式推荐保存为个人词库旁的 `words.feedback.json`（version 1），每条仅 text、syllables、value（1或-1），0为移除。不是学习记录：无次数、时间戳或使用轨迹，不创建未确认词的 words.json 记录。原 version-1 words.json 和优先侧文件格式不变。所有管理操作共用 words.lock，校验大小/条数/身份、拒绝符号链接及损坏文件，原子替换文件；不重建损坏词库。

个人词库显示优先／减少／自动。仅设置过推荐但未使用的词显示“—”次数，可恢复自动、取消优先或移除设置；删除已有个人词也移除其推荐设置。恢复自动会同时移除该词原有的显式优先标记。暂停自动记忆不阻止用户显式调整推荐。

Mac 菜单携带输入、候选及引擎会话快照，点击时重新验证；后台执行有时限的本地管理请求。异步完成只有仍处在同一输入修订和会话时才刷新候选，失败给出可重试提示。不提交文字、不伪造选择回执、不增加使用次数，不恢复已经结束的拼音。原数字选择、回车英文、Tab译法和Shift切换不变。

## 检查

```sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
runtime/python/bin/python prototype/tests/candidate_feedback_test.py
native-macos/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --candidate-feedback-test
native-macos/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-learning --preview-duration 300
```

测试和预览只用临时词库。检查包括显式设置不学习、即时排序、重启保留、管理撤销、符号/过期会话拒绝、非法参数及损坏/符号链接保护。原60个准确性用例保持54首选、58召回，无排名退步。这里只说明工程回归，不代表新增通用准确率收益。

Rust管理协议增加显式 `feedback` action（text、syllables、feedback），list响应增加feedback整数并显示零使用反馈词；候选响应增加可选feedbackEligible/feedback字段。Mac专用排序使用新设置；legacy CLI和Windows默认排名不启用反馈，已用本地默认路径对照验证。Windows构建/真实设备未在本轮执行，Windows界面未接入新操作。

代码、测试与规则随项目GPL-3.0-or-later；未增加外部语料、模型、生产依赖或网络服务。
