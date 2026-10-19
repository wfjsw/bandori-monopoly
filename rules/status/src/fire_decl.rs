//! 火罐 declaration registry -- one source of truth for each character skill's
//! 「（初始N，上限M）」 face parenthetical.
//!
//! MARKERS.md §7: the face printing the numbers is the owner of the pool; the
//! registry is what `Sumimi:Sumimi是二人一体的` and `skill:若叶睦:天生的演员`
//! re-declare from, instead of hardcoded tables. Lives next to the fire host
//! (`rules/status`) so every guest -- skill bodies, band skills, hand cards --
//! reads the same rows.
//!
//! Each row quotes the face. A skill with no row owns no fire pots (cap 0).

/// One face's fire-pot declaration.
pub struct FireDecl {
    /// The character-skill rule id (`skill:…`).
    pub skill: &'static str,
    /// 「初始N」 -- the count created at `DeckAtGameStart`.
    pub initial: i32,
    /// 「上限M」 -- the held-cap invariant (MARKERS.md I2 「在任意时刻」).
    pub cap: i32,
    /// The face parenthetical, for the consistency tests and the log.
    pub face: &'static str,
}

/// Every skill that prints a fire-pot parenthetical. Keyed by skill id.
pub const FIRE_DECLS: &[FireDecl] = &[
    FireDecl { skill: "skill:千早爱音:重新开始", initial: 2, cap: 3, face: "（初始2，上限3）" },
    FireDecl { skill: "skill:市谷有咲:盆栽爱好者", initial: 0, cap: 2, face: "（初始0，上限2）" },
    FireDecl { skill: "skill:户山香澄:非凡之星", initial: 0, cap: 1, face: "（初始0，上限1）" },
    FireDecl { skill: "skill:上原绯玛丽:大家一起迈出新的一步", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:八幡海铃:熟练的支援贝斯手", initial: 0, cap: 4, face: "（初始0，上限4）" },
    FireDecl { skill: "skill:桐谷透子:天上天下，唯我独尊", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:凑友希那:来练习吧", initial: 0, cap: 1, face: "（初始0，上限1）" },
    FireDecl { skill: "skill:今井莉莎:慈爱女神", initial: 2, cap: 2, face: "（初始2，上限2）" },
    FireDecl { skill: "skill:纯田真奈:甜甜圈爱好者", initial: 1, cap: 2, face: "（初始1，上限2）" },
    FireDecl { skill: "skill:奥泽美咲:另一个我", initial: 0, cap: 2, face: "（初始0，上限2）" },
    FireDecl { skill: "skill:青叶摩卡:我行我素", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:祐天寺若麦:大主播喵梦亲", initial: 0, cap: 5, face: "（初始0，上限5）" },
    FireDecl { skill: "skill:若叶睦（CRYCHIC）:精致的人偶", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:广町七深:这是很普通的事吧？", initial: 2, cap: 2, face: "（初始2，上限2）" },
    FireDecl { skill: "skill:和奏瑞依:一次又一次竭尽全力", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:美竹兰:叛逆的红挑染", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:要乐奈:投币式停车场的猫", initial: 3, cap: 3, face: "（初始3，上限3）" },
    FireDecl { skill: "skill:牛込里美:里美的决心", initial: 1, cap: 3, face: "（初始1，上限3）" },
    FireDecl { skill: "skill:白金燐子:即使1cm也要前进", initial: 3, cap: 3, face: "（初始3，上限3）" },
    FireDecl { skill: "skill:山吹沙绫:焕然一新的天空中", initial: 3, cap: 5, face: "（初始3，上限5）" },
    FireDecl { skill: "skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？", initial: 1, cap: 2, face: "（初始1，上限2）" },
    FireDecl { skill: "skill:丰川祥子:请把你们的人生交给我", initial: 0, cap: 3, face: "（初始0，上限3）" },
    FireDecl { skill: "skill:佐藤益木:与燃烧的红色一起驰骋", initial: 0, cap: 2, face: "（初始0，上限2）" },
    FireDecl { skill: "skill:冰川纱夜:踏上荆棘之路的觉悟", initial: 10, cap: 10, face: "（初始10，上限10）" },
    FireDecl { skill: "skill:长崎素世:通透的颜色", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:花园多惠:花园警察，出警！", initial: 2, cap: 4, face: "（初始2，上限4）" },
    FireDecl { skill: "skill:椎名立希:决定练习日的会议", initial: 0, cap: 5, face: "（初始0，上限5）" },
    FireDecl { skill: "skill:宇田川巴:豚骨酱油拉面大姐", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:高松灯（CRYCHIC）:跌跌撞撞...", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:高松灯:诗超绊", initial: 4, cap: 4, face: "（初始4，上限4）" },
    FireDecl { skill: "skill:羽泽鸫:伟大的平凡", initial: 1, cap: 1, face: "（初始1，上限1）" },
    FireDecl { skill: "skill:都筑诗船:尽力了吗", initial: 1, cap: 2, face: "（初始1，上限2）" },
    FireDecl { skill: "skill:三角初华（Sumimi）:成为偶像", initial: 2, cap: 2, face: "（初始2，上限2）" },
    // Cap-only faces: 「（上限7）」 / 「（火罐上限3）」 -- no 初始, so the pool
    // starts empty. kaoru's cap holds from the first turn; uika's is the 状态2
    // cap (`skill:三角初华:Imprisoned XII` writes it on entering 状态2).
    FireDecl { skill: "skill:濑田薰:梦幻的王子殿下", initial: 0, cap: 7, face: "（上限7）" },
    FireDecl { skill: "skill:三角初华:Imprisoned XII", initial: 0, cap: 3, face: "（火罐上限3）" },
];

/// The declaration row for `skill`, or `None` (no fire pots / unknown face).
pub fn decl_of(skill: &str) -> Option<&'static FireDecl> {
    FIRE_DECLS.iter().find(|d| d.skill == skill)
}