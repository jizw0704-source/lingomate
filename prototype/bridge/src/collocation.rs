//! Independently authored grammatical compatibility priors, GPL-3.0-or-later.
//! Not corpus frequencies or copied language-model weights. No host context.
const VERBS: &[&str] = &[
    "研究", "分析", "测试", "测量", "提高", "改善", "优化", "开发", "学习", "使用", "选择", "需要",
    "发现", "解决", "讨论", "准备", "完成", "检查",
];
const OBJECTS: &[&str] = &[
    "材料",
    "性能",
    "方法",
    "问题",
    "结果",
    "数据",
    "系统",
    "功能",
    "软件",
    "词语",
    "方案",
    "实验",
    "技术",
    "设备",
    "模型",
    "效率",
    "准确性",
    "质量",
];
const PEOPLE: &[&str] = &["我", "我们", "你", "你们", "他", "他们", "大家"];
const TIME: &[&str] = &["今天", "明天", "昨天", "现在", "以后", "最近"];

pub fn score(left: &str, right: &str) -> f64 {
    if (VERBS.contains(&left) && OBJECTS.contains(&right))
        || (PEOPLE.contains(&left) && (VERBS.contains(&right) || right == "一起"))
        || (TIME.contains(&left) && PEOPLE.contains(&right))
        || (left == "一起" && ["学习", "研究", "讨论", "工作", "打", "去"].contains(&right))
        || (left == "打" && ["羽毛球", "乒乓球", "篮球", "排球", "网球"].contains(&right))
    {
        1.2
    } else {
        0.0
    }
}

/// Bounded confidence: repeated choices gain gradually; stale preferences lose strength.
pub fn preference_bonus(count: u32, age: u64) -> f64 {
    if count < 2 {
        return 0.0;
    }
    0.8 * f64::from(count.min(20)).ln_1p() / (1.0 + age as f64 / 256.0)
}

/// Independently authored compatibility rules, not a corpus or a text generator.
/// No new candidates: callers only apply this to exact whole-word homophones.
pub fn context_score(prefix: &str, word: &str) -> f64 {
    if prefix.is_empty() {
        return 0.0;
    }
    const PAIRS: &[(&str, &str)] = &[
        ("数学", "公式"),
        ("达成", "共识"),
        ("发布", "公示"),
        ("公开", "公示"),
        ("临床", "试验"),
        ("化学", "实验"),
        ("物理", "实验"),
        ("客观", "事实"),
        ("开始", "实施"),
        ("测量", "电导"),
        ("提出", "异议"),
        ("具有", "意义"),
        ("增强", "记忆"),
        ("精湛", "技艺"),
        ("公民", "权利"),
        ("行政", "权力"),
        ("运行", "程序"),
        ("提前", "预见"),
        ("遵守", "纪律"),
        ("创下", "纪录"),
        ("面临", "挑战"),
        ("表达", "谢意"),
        ("两国", "协议"),
        ("参加", "会议"),
        ("提出", "建议"),
        ("天气", "预报"),
        ("申请", "专利"),
        ("办理", "手续"),
        ("制定", "计划"),
        ("认真", "计划"),
        ("学习", "知识"),
        ("部门", "职责"),
        ("拥有", "资金"),
        ("规范", "管理"),
    ];
    if PAIRS
        .iter()
        .any(|(left, right)| word == *right && prefix.ends_with(left))
    {
        return 1.6;
    }
    VERBS
        .iter()
        .chain(PEOPLE)
        .chain(TIME)
        .chain(["一起", "打"].iter())
        .filter(|left| prefix.ends_with(**left) && (left.chars().count() > 1 || prefix == **left))
        .map(|left| score(left, word))
        .fold(0.0_f64, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn priors_are_bounded_and_directional() {
        assert_eq!(score("研究", "材料"), 1.2);
        assert_eq!(score("材料", "研究"), 0.0);
        assert_eq!(score("AI", "材料"), 0.0);
        assert_eq!(preference_bonus(1, 0), 0.0);
        assert!(preference_bonus(3, 0) > preference_bonus(2, 0));
        assert!(preference_bonus(2, 4096) < preference_bonus(2, 1));
    }
}
