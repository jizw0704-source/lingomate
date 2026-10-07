"""原型用法示例，不作为完整词典。"""

import json
from pathlib import Path

entries = {
    "学习": [
        (
            "study",
            "v.",
            "有计划地读书或研究，强调学习过程。",
            "I study English every evening.",
        ),
        ("learn", "v.", "获得知识或技能，强调学会和掌握。", "I want to learn to swim."),
        ("learning", "n.", "指学习这个过程，用在名词位置。", "Learning takes time."),
    ],
    "喜欢": [
        ("like", "v.", "一般的喜欢，适用于事物或活动。", "I like this book."),
        ("enjoy", "v.", "享受某项活动，常接动词 -ing。", "I enjoy reading."),
        ("be fond of", "phr.", "对某人或某物有喜爱之情。", "She is fond of music."),
    ],
    "方便": [
        (
            "convenient",
            "adj.",
            "时间、地点或安排便利。",
            "Is this time convenient for you?",
        ),
        ("handy", "adj.", "工具实用，或东西在手边容易使用。", "This tool is handy."),
    ],
    "苹果": [("apple", "n.", "水果苹果；一个苹果常用 an apple。", "I ate an apple.")],
    "开发": [
        ("develop", "v.", "开发软件、产品或能力。", "We develop software."),
        (
            "exploit",
            "v.",
            "开发利用资源；也可能含剥削或过度利用之意。",
            "They exploit mineral resources.",
        ),
        (
            "development",
            "n.",
            "开发这一过程，用在名词位置。",
            "Software development takes time.",
        ),
    ],
    "银行": [("bank", "n.", "提供金融服务的机构。", "The bank opens at nine.")],
    "行": [
        ("OK", "int.", "表示同意，例如“行，就这样”。", "OK, let us try."),
        ("work", "v.", "表示办法可行，例如“这个方法行”。", "This approach works."),
    ],
    "工作": [
        ("work", "n.", "工作这一活动，通常不可数。", "I have work to do."),
        ("job", "n.", "一份工作或具体任务，可数。", "She has a new job."),
        (
            "operate",
            "v.",
            "机器或系统正常运转的“工作”。",
            "The system operates normally.",
        ),
    ],
    "问题": [
        ("problem", "n.", "需要解决的困难或故障。", "We need to solve this problem."),
        ("question", "n.", "需要回答的疑问或提问。", "May I ask a question?"),
        ("issue", "n.", "需要讨论或处理的议题。", "We discussed the issue."),
    ],
    "明天": [("tomorrow", "adv.", "在明天，可作时间副词。", "We will meet tomorrow.")],
    "今天": [("today", "adv.", "在今天，可作时间副词。", "I am working today.")],
    "你好": [
        ("hello", "int.", "通用问候。", "Hello, nice to meet you."),
        ("hi", "int.", "更随意的问候。", "Hi, how are you?"),
    ],
    "朋友": [("friend", "n.", "朋友，复数是 friends。", "She is my friend.")],
    "可以": [
        ("can", "v.", "能力或许可，后接动词原形。", "You can sit here."),
        ("may", "v.", "表示许可时较正式。", "May I come in?"),
    ],
    "需要": [
        ("need", "v.", "需要某物或做某事。", "I need some help."),
        ("require", "v.", "条件或规则要求，较正式。", "This task requires patience."),
    ],
    "知道": [("know", "v.", "知道事实或如何做某事。", "I know the answer.")],
    "研究": [
        ("research", "v.", "系统调查一个问题或领域。", "We research new materials."),
        ("study", "v.", "仔细考察对象，也可指学习。", "We study battery performance."),
    ],
    "材料": [
        (
            "material",
            "n.",
            "原料或物质，种类常用复数 materials。",
            "This material is lightweight.",
        )
    ],
    "电池": [
        (
            "battery",
            "n.",
            "电池或电池组，复数 batteries。",
            "The battery needs charging.",
        )
    ],
    "会议": [
        ("meeting", "n.", "工作讨论或日常会议。", "The meeting starts at ten."),
        (
            "conference",
            "n.",
            "通常规模更大的专业会议。",
            "She attended a scientific conference.",
        ),
    ],
    "计划": [
        ("plan", "n.", "一个计划或安排。", "We have a plan."),
        ("plan", "v.", "计划做某事。", "We plan to start tomorrow."),
    ],
    "设计": [
        ("design", "v.", "设计产品或方案。", "We design useful tools."),
        ("design", "n.", "设计方案或设计本身。", "The design is simple."),
    ],
    "重要": [
        ("important", "adj.", "重要的。", "This is an important decision."),
        (
            "significant",
            "adj.",
            "具有明显意义或影响；科研中需看具体含义。",
            "The result is significant.",
        ),
    ],
    "时间": [("time", "n.", "时间，问几点常用 What time。", "We need more time.")],
    "了解": [
        ("understand", "v.", "理解意思或情况。", "I understand the problem."),
        (
            "learn about",
            "phr.",
            "获取信息，了解新主题。",
            "I want to learn about this project.",
        ),
    ],
    "决定": [
        ("decide", "v.", "作出决定，常接 to 或 on。", "We decided to leave."),
        (
            "decision",
            "n.",
            "一个决定，用在名词位置。",
            "That was a difficult decision.",
        ),
    ],
    "免费": [
        ("free", "adj.", "免费的，也可指自由，需看语境。", "Admission is free."),
        (
            "free of charge",
            "phr.",
            "明确表示不收费。",
            "The service is free of charge.",
        ),
    ],
    "支持": [
        ("support", "v.", "支持某人、观点或功能。", "I support your idea."),
        ("support", "n.", "支持或帮助，用在名词位置。", "Thank you for your support."),
    ],
    "选择": [
        ("choose", "v.", "选择，后接对象。", "Choose a language."),
        ("select", "v.", "从一组对象中挑选，较正式。", "Select a candidate."),
        ("choice", "n.", "选择或选项，用在名词位置。", "You have a choice."),
    ],
    "谢谢": [
        ("thank you", "phr.", "完整的感谢表达。", "Thank you for your help."),
        ("thanks", "int.", "更随意的感谢。", "Thanks for coming."),
    ],
}
result = {
    word: [
        dict(zip(("word", "pos", "note", "example"), sense, strict=True))
        for sense in senses
    ]
    for word, senses in entries.items()
}
assert len(result) == 30
Path(__file__).with_name("details.json").write_text(
    json.dumps(result, ensure_ascii=False, indent=2) + "\n"
)
