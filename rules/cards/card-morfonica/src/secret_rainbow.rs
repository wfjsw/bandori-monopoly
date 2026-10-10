//! `Mor:秘密与青春的虹彩` -- C# `CardSecretRainbow` (MatchHost.cs:5178-5220): halve a
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:秘密与青春的虹彩`）:
//! > 秘密与青春的虹彩：[反击] 
//! > （1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。
//! > （2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。
//!
//! payment to a junior peer, or boost a senior peer's payment to you.
//!
//! Counteraction-only (`Normal => false`). Grades are static data shipped with
//! the rule (see [`GRADES`] below).

use card_sdk::abi::ChainKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const SECRET_RAINBOW: CardDef = CardDef::new(
    "Mor:秘密与青春的虹彩",
    &[On::Counteract(
        &[ChainKind::Effect],
        // 规则书[反击]: 「支付」 -- a player-to-player payment of a positive
        // amount. A print (game -> player, `actor == -1`) is a [获得], not a
        // 「支付」, and must not open this window; nor may the causer's own
        // `abnormal` (`actor` = the causer). `value > 0` also covers C#
        // `!t.Pay.cancel` -- a payment an earlier counteraction already
        // reduced to 0 reads as `value() == 0`.
        "value > 0 && chain_has(Pay) && actor >= 0 && target >= 0 && actor != target",
        Some(can_counteract),
        counteract,
    )],
);

// ============ NORMALIZED GRADE ORDINALS ================================
// Ruling 2026-10-06: grades come from BanG Dream character facts, shipped as
// static data accompanying the rule. Grades advance for everyone at the same
// time, so the relative order (same / higher / lower) is static.
//
// Everyone is normalized into ONE reference school year -- the MyGO / Ave
// Mujica era (the wiki's `Year_S3` column) -- and converted to an absolute
// ordinal: jr-hi 1=7, jr-hi 2=8, jr-hi 3=9, hi 1=10, hi 2=11, hi 3=12,
// university / adult = 13+. Comparing ordinals gives the card's relations
// directly:
//   学妹 (junior)  = strictly lower ordinal
//   同级生 (same)  = equal ordinal
//   学姐 (senior)  = strictly higher ordinal
//
// Sources: https://bandori.fandom.com/wiki/<wiki page>, the `Infobox
// character` `School_S1/S2/S3` / `Year_S1/S2/S3` fields plus the lead prose,
// fetched 2026-10-06 via the MediaWiki API (`action=parse&prop=wikitext`).
// Band debut offsets from the `School_S*` column presence per band page and
// https://bandori.fandom.com/wiki/Morfonica ("second-year students").
//
// Corrections vs a naive debut-year table (see `crates/game-rules/tests/
// rb_morfonica.rs` for the full note): 鳰原令王那 is junior-high 3rd yr = 9,
// not high school; 珠手知由 is 12th grade = 12; 和奏瑞依 / 佐藤益木 are 3rd-yr
// high school = 12; 长崎素世 is Tsukinomori 1st yr = 10.
//
// Keyed by the character's Chinese name (`ctx::character_is`); `cnId` is the
// id from `data/characters.json`, the wiki page is the cited source.
const GRADES: &[(&str, &str, i32)] = &[
    // (name, cnId, ordinal)
    ("户山香澄", "001", 12),   // Toyama_Kasumi, Hanasakigawa Hi 3rd
    ("花园多惠", "002", 12),   // Hanazono_Tae, Hanasakigawa Hi 3rd
    ("牛込里美", "003", 12),   // Ushigome_Rimi, Hanasakigawa Hi 3rd
    ("山吹沙绫", "004", 12),   // Yamabuki_Saaya, Hanasakigawa Hi 3rd
    ("市谷有咲", "005", 12),   // Ichigaya_Arisa, Hanasakigawa Hi 3rd
    ("美竹兰", "006", 12),     // Mitake_Ran, Haneoka Hi 3rd
    ("青叶摩卡", "007", 12),   // Aoba_Moca, Haneoka Hi 3rd
    ("上原绯玛丽", "008", 12), // Uehara_Himari, Haneoka Hi 3rd
    ("宇田川巴", "009", 12),   // Udagawa_Tomoe, Haneoka Hi 3rd
    ("羽泽鸫", "010", 12),     // Hazawa_Tsugumi, Haneoka Hi 3rd
    ("丸山彩", "011", 13),     // Maruyama_Aya, Yotsuba Univ 1st
    ("冰川日菜", "012", 13),   // Hikawa_Hina, Keiho Univ 1st
    ("白鹭千圣", "013", 13),   // Shirasagi_Chisato, Yotsuba Univ 1st
    ("大和麻弥", "014", 13),   // Yamato_Maya, Keiho Univ 1st
    ("若宫伊芙", "015", 12),   // Wakamiya_Eve, Hanasakigawa Hi 3rd
    ("凑友希那", "016", 13),   // Minato_Yukina, Yotsuba Univ 1st
    ("冰川纱夜", "017", 13),   // Hikawa_Sayo, Keiho Univ 1st
    ("今井莉莎", "018", 13),   // Imai_Lisa, Yotsuba Univ 1st
    ("白金燐子", "019", 13),   // Shirokane_Rinko, Yotsuba Univ 1st
    ("宇田川亚子", "020", 11), // Udagawa_Ako, Haneoka Hi 2nd
    ("弦卷心", "021", 12),     // Tsurumaki_Kokoro, Hanasakigawa Hi 3rd
    ("濑田薰", "022", 13),     // Seta_Kaoru, Yotsuba Univ 1st
    ("北泽育美", "023", 12),   // Kitazawa_Hagumi, Hanasakigawa Hi 3rd
    ("松原花音", "024", 13),   // Matsubara_Kanon, Keiho Univ 1st
    ("奥泽美咲", "025", 12),   // Okusawa_Misaki, Hanasakigawa Hi 3rd
    ("仓田真白", "026", 11),   // Kurata_Mashiro, Tsukinomori Hi 2nd
    ("桐谷透子", "027", 11),   // Kirigaya_Touko, Tsukinomori Hi 2nd
    ("广町七深", "028", 11),   // Hiromachi_Nanami, Tsukinomori Hi 2nd
    ("二叶筑紫", "029", 11),   // Futaba_Tsukushi, Tsukinomori Hi 2nd
    ("八潮瑠唯", "030", 11),   // Yashio_Rui, Tsukinomori Hi 2nd
    ("和奏瑞依", "031", 12),   // Wakana_Rei, Geijutsu Academy 3rd
    ("朝日六花", "032", 11),   // Asahi_Rokka, Haneoka Hi 2nd
    ("佐藤益木", "033", 12),   // Satou_Masuki, Shirayuki Private Hi 3rd
    ("鳰原令王那", "034", 9),  // Nyubara_Reona, Kamogawa Jr-Hi 3rd
    ("珠手知由", "035", 12),   // Tamade_Chiyu, Celosia Intl 3rd/12th gr
    ("高松灯", "036", 10),     // Takamatsu_Tomori, Haneoka Hi 1st
    ("千早爱音", "037", 10),   // Chihaya_Anon, Haneoka Hi 1st
    ("要乐奈", "038", 9),      // Kaname_Raana, Hanasakigawa Jr-Hi 3rd
    ("长崎素世", "039", 10),   // Nagasaki_Soyo, Tsukinomori Hi 1st
    ("椎名立希", "040", 10),   // Shiina_Taki, Hanasakigawa Hi 1st
    ("三角初华", "041", 10),   // Misumi_Uika, Hanasakigawa Hi 1st
    ("若叶睦", "042", 10),     // Wakaba_Mutsumi, Tsukinomori Hi 1st
    ("丰川祥子", "043", 10),   // Togawa_Sakiko, Haneoka Hi 1st
    ("八幡海铃", "044", 10),   // Yahata_Umiri, Hanasakigawa Hi 1st
    ("祐天寺若麦", "045", 10), // Yuutenji_Nyamu, Geijutsu Academy 1st
    // Adults (no school year on the wiki): treat as 13+ so every student is a
    // 学妹 to them and no student is a 学姐.
    ("纯田真奈", "046", 13),   // Sumita_Mana (adult)
    ("月岛麻里奈", "047", 13), // Tsukishima_Marina (CiRCLE staff, adult)
    ("都筑诗船", "048", 13),   // Tsuzuki_Shifune (SPACE owner, adult)
];

/// The normalized grade ordinal of `player_id`'s character, or `None` when the
/// character is not in the table.
fn grade_of(player_id: i32) -> Option<i32> {
    GRADES
        .iter()
        .find(|(name, _, _)| ctx::character_is(player_id, name))
        .map(|&(_, _, ord)| ord)
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]（1）: 「当你向学妹或同级生支付时」
    // 规则书[反击]（2）: 「当学姐或同级生向你支付的时候」
    // The payment's shape (a positive `Pay` between two distinct players) is
    // the pre; the grade-direction comparison is the residual.
    let Some(my) = grade_of(player_id) else {
        return false;
    };
    let from = trigger::player_id();
    let to = trigger::target();
    // （1）「当你向学妹或同级生支付时」 -- the holder is the payer and the payee
    // is junior-or-same (payee grade <= holder grade).
    if from == player_id {
        return grade_of(to).is_some_and(|g| g <= my);
    }
    // （2）「当学姐或同级生向你支付的时候」 -- the holder is the payee and the
    // payer is senior-or-same (payer grade >= holder grade).
    if to == player_id {
        return grade_of(from).is_some_and(|g| g >= my);
    }
    false
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value() as i64;
    let from = trigger::player_id();
    // 规则书[反击]（1）: 「此次支付金额减半」 -- C# `pay.amount = CeilTo(amount / 2.0, 10)`.
    // 规则书[反击]（2）: 「使此次支付资金变成1.5倍」 -- C# `pay.amount = CeilTo(amount * 1.5, 10)`.
    // `CeilTo` rounds up to a multiple of 10.
    if from == player_id {
        let half = (amount + 1) / 2;
        let half = ((half + 9) / 10) * 10;
        trigger::set_pay_amount(half as i32);
        ctx::log(
            player_id,
            &Msg::new(key!("secret_rainbow_half"))
                .n("money", amount)
                .n("n", half),
        );
    } else {
        let boosted = (amount * 3 + 1) / 2;
        let boosted = ((boosted + 9) / 10) * 10;
        trigger::set_pay_amount(boosted as i32);
        ctx::log(
            player_id,
            &Msg::new(key!("secret_rainbow_boost"))
                .n("money", amount)
                .n("n", boosted),
        );
    }
    Ok(())
}