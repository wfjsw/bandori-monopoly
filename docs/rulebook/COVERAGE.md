# Behavioural coverage of the card / skill rules

Generated from the live sheet extracts `target/scratch/rb/*.md`, the
glossary `data/rules.txt`, the suites `crates/game-rules/tests/rb_*.rs`
+ `live_match.rs` + `ruleset.rs`, the design docs
`docs/rulebook/CROSS-TESTS.md` / `TEST-FINDINGS.md`, and the built
ruleset manifest (`target/scratch/rb/manifest.txt`, 250 declared cards /
skills). Black-box: `rules/cards/`, `rules/skills/`, `rules/fixtures/`
and `target/scratch/tainted/` were never opened.

Status codes: **ok** = covered-passing · **ign** = covered-ignored ·
**shl** = shallow (the test only checks presence / binding / playability)
· **unc** = uncovered · **unt** = untestable (harness or ABI gap).

## Summary

Current suite state (2026-10-06 working tree; grep of the `#[test]` /
`#[ignore` **attributes** per file under `crates/game-rules/tests/`,
comment mentions excluded): **774 tests, 102 ignored**. Ignore kinds:
`DISCREPANCY` × 84, `RULING` × 12, `CROSS-AGENT` × 1, `TODO(ABI)` × 2,
`TODO(harness)` × 1, plus 2 harness soaks (`q4_unintended`'s soak / report
generator). Whole-tree detail is in
[TEST-FINDINGS.md](TEST-FINDINGS.md) §Status.

`rb_guards.rs` (the [反击]-guard work) **has landed**: 10 tests, 0 ignored,
all green. It checks each `can_counteract` guard matches exactly what the
card's rulebook clause names -- the money pipeline opens a `ChainKind::Effect`
window on **every** money movement, so a guard with no effect-kind check
would fire on a plain gain. Clauses covered: `AG:宣战布告#1`,
`Mor:迷茫之蝶们的三全音#1/#4`, `AG:(兰) 像往常一样#1/#2`, `MyGO:普通与理所当然`,
`R:选择自己的舞台`, `Mor:离心力，不为所动`, `Mor:秘密与青春的虹彩`,
`R:（亚子）黑暗大魔姬亚子`, `Mor:（小白）`, and the [眩晕] / [异常移动效果]
gates.

The clause-matrix counts below are **as of 2026-10-06 HEAD** (before the
match-start / per-draw hooks, the money pipeline, the band-crystal FieldCard
and the property map landed in the working tree). They were derived by
walking each sheet clause against the test names; recomputing them needs
that walk again, so treat the percentages as a baseline, not a live figure.
Where a row's `tests` names a test that is no longer `#[ignore]`d, the code
is greener than the row says -- the `#[ignore]` reasons in
`crates/game-rules/tests/` are authoritative (see [TEST-FINDINGS.md](TEST-FINDINGS.md) §4).

**2026-10-06 assertion audit.** Several tests previously only `eprintln!`d an
observed value and were counted as **ok**. Each now carries a real assertion;
where the engine disagrees the test is `#[ignore = "DISCREPANCY: …"]`, and
where the sheet text does not decide the value `#[ignore = "RULING: …"]`.
Reclassified rows (per-clause rows below are authoritative; the group counts
above still reflect the pre-audit baseline): `CRYCHIC:如果能一直持续下去...#2`
ok→ign, `R:（燐子）Ringing Bloom#2` ok→ign, `Sumimi:Sweet Escape#1` ok→ign,
`skill:北泽育美:全垒打！#1/#2` ok→unc, `…#3` ok→shl, and
`Mujica:欢迎来到ave mujica的世界#1` ign→ok. `PPP:Tomorrow's Door#3` is now
genuinely covered: both `t06_tomorrows_door_surcharge` and
`ix_door_surcharge_on_rent` assert `paid = R + houses×100`.

| group | rules | clauses | ok | ign | shl | unc | unt | %ok | %shl | %unc |
|---|---|---|---|---|---|---|---|---|---|---|
| Afterglow (AG) | 21 | 78 | 40 | 2 | 5 | 31 | 0 | 51% | 6% | 40% |
| CRYCHIC | 21 | 57 | 20 | 6 | 10 | 21 | 0 | 35% | 18% | 37% |
| 通用 / CiRCLE | 17 | 55 | 47 | 3 | 2 | 2 | 1 | 85% | 4% | 4% |
| Hello, Happy World! | 21 | 66 | 30 | 2 | 1 | 33 | 0 | 45% | 2% | 50% |
| Morfonica | 21 | 60 | 42 | 10 | 0 | 8 | 0 | 70% | 0% | 13% |
| Ave Mujica | 21 | 61 | 27 | 7 | 3 | 24 | 0 | 44% | 5% | 39% |
| MyGO!!!!! | 21 | 69 | 50 | 8 | 0 | 11 | 0 | 72% | 0% | 16% |
| Pastel*Palettes | 25 | 82 | 60 | 3 | 2 | 17 | 0 | 73% | 2% | 21% |
| Poppin' Party | 24 | 61 | 43 | 2 | 0 | 16 | 0 | 70% | 0% | 26% |
| RAISE A SUILEN | 21 | 53 | 40 | 3 | 0 | 10 | 0 | 75% | 0% | 19% |
| Roselia | 22 | 61 | 30 | 8 | 1 | 22 | 0 | 49% | 2% | 36% |
| Sumimi | 14 | 34 | 15 | 1 | 2 | 16 | 0 | 44% | 6% | 47% |
| **total** | **249** | **737** | **444** | **55** | **26** | **211** | **1** | **60%** | **4%** | **29%** |

Overall (as of 2026-10-06 HEAD, same basis as the table):
**444 / 737 = 60%** covered-passing, **55** covered-ignored, **26** shallow,
**211** uncovered, **1** untestable.

## Per-card clause matrix

One line per clause. `tests` names the test functions that genuinely
assert the clause (or the reason it is not).

### Afterglow (AG)

| clause | meaning | status | tests |
|---|---|---|---|
| `AG:宣战布告#1` | [反击] trigger: you/your tile hit by another's card effect | **ok** | declaration_offered_against_targeting_card |
| `AG:宣战布告#2` | [指定] the acting player | **ok** | declaration_pays_500_and_draws |
| `AG:宣战布告#3` | target pays user 500 | **ok** | declaration_pays_500_and_draws,ix_declaration_vs_budokan,ix_declaration_vs_rain |
| `AG:宣战布告#4` | user draws 1 | **ok** | declaration_pays_500_and_draws |
| `AG:Y.O.L.O#1` | [反击] timing: your roll before settle | **ok** | yolo_counteracts_only_to_rolls_and_adds_1d4,yolo_adds_1d4_to_the_roll |
| `AG:Y.O.L.O#2` | result += 1d4 face | **ok** | yolo_adds_1d4_to_the_roll,m01_yolo_own_roll_only,m04_minus_two_plus_yolo |
| `AG:Y.O.L.O#3` | own-roll only (live-sheet wording) | **ok** | yolo_counteracts_only_to_rolls_and_adds_1d4,m01_yolo_own_roll_only |
| `AG:商店街的青梅竹马#1` | roll 1d6 | **ok** | childhood_teleports_to_nth_shop_tile |
| `AG:商店街的青梅竹马#2` | [传送] to the nth owned shop-street tile | **ok** | childhood_teleports_to_nth_shop_tile |
| `AG:商店街的青梅竹马#3` | if roll > owned count -> shop street itself | **ok** | childhood_over_roll_goes_to_shotengai |
| `AG:商店街的青梅竹马#4` | counts as the main move | **ok** | childhood_teleports_to_nth_shop_tile |
| `AG:即使夕阳落山#1` | [限] gate: X halved before round 100 | **ok** | sunset_x_halved_before_round_100,sunset_is_playable |
| `AG:即使夕阳落山#2` | [指定] four house tiles + all other players | **shl** | sunset_is_playable (playable only; designation unasserted) |
| `AG:即使夕阳落山#3` | X = ceil10((deed+houses)/alive-others) | **ok** | sunset_x_halved_before_round_100 |
| `AG:即使夕阳落山#4` | delete 1 house per designated tile | **shl** | ix_band_house_then_sunset_demolish (demolish side only) |
| `AG:即使夕阳落山#5` | designated players pay user X | **unc** | no test asserts the X payment |
| `AG:(兰) 像往常一样#1` | may be used as a [反击] | **ok** | ran_card_is_exclusive |
| `AG:(兰) 像往常一样#2` | after an abnormal move: return to start at turn end, cancel all effects | **ok** | m08_ran_undoes_forced_move (ran_card_returns_to_start_after_abnormal only pins the [停留] decay; its setup cannot show the return-to-start) |
| `AG:（摩卡）0.5倍速#1` | place on field, gain 3 miracle crystals | **ok** | moca_card_places_with_three_crystals |
| `AG:（摩卡）0.5倍速#2` | turn end: remove 1 crystal; 0 -> discard | **ok** | moca_card_places_with_three_crystals |
| `AG:（摩卡）0.5倍速#3` | on field: move-roll final /2 (ceil) | **ok** | moca_card_halves_move_and_spend,ix_encore_and_moca_half_speed |
| `AG:（摩卡）0.5倍速#4` | on field: all pays and spends halved | **ok** | moca_card_halves_move_and_spend,c20_vocal_too_hard_plus_half_speed,m05_repaint_plus_half_speed |
| `AG:无论是何种颜色的夕阳#1` | roll 1d6 and branch | **unc** | no test at all |
| `AG:无论是何种颜色的夕阳#2` | face 1: gain 1500 | **unc** | — |
| `AG:无论是何种颜色的夕阳#3` | face 2: discard-pile card to draw-pile top | **unc** | — |
| `AG:无论是何种颜色的夕阳#4` | face 3: draw 1 | **unc** | — |
| `AG:无论是何种颜色的夕阳#5` | face 4: refill fire pots OR gain 2000 | **unc** | — |
| `AG:无论是何种颜色的夕阳#6` | face 5: discard-pile card to hand | **unc** | — |
| `AG:无论是何种颜色的夕阳#7` | face 6: trigger 3 of faces 1-5 | **unc** | — |
| `AG:无论是何种颜色的夕阳#8` | face >6 wraps and recounts from 1 | **unc** | — |
| `AG:回家的路上绕个道#1` | may be used as a [反击] | **ok** | detour_can_be_a_counter |
| `AG:回家的路上绕个道#2` | Afterglow-style move (Afterglow skill (2)) | **unc** | not asserted; depends on an unported Afterglow (2) |
| `AG:回家的路上绕个道#3` | next move roll becomes 1d6 | **unc** | — |
| `AG:ONE OF US#1` | place on field | **ok** | one_of_us_places_on_field |
| `AG:ONE OF US#2` | choose another Afterglow char / shop-street owner | **shl** | one_of_us_places_on_field |
| `AG:ONE OF US#3` | designate one deed each | **unc** | — |
| `AG:ONE OF US#4` | share settlement income | **ign** | t05_one_of_us_shares (DISCREPANCY: shared-settlement arrangement) |
| `AG:ONE OF US#5` | no third party in the share | **ign** | t05_one_of_us_shares |
| `AG:ONE OF US#6` | shop-street tiles preferred | **unc** | — |
| `AG:ONE OF US#7` | original owner settles first, then the share is added (unaffected) | **unc** | — |
| `AG:ONE OF US#8` | on bankruptcy the two deeds move to the survivor | **unc** | — |
| `AG:朝同一片天空迈进#1` | [反击]: auto-play on draw | **ok** | sky_fires_on_draw |
| `AG:朝同一片天空迈进#2` | hand >= 3: gain hand*600 | **ok** | sky_fires_on_draw |
| `AG:朝同一片天空迈进#3` | hand < 3: draw 1 | **ok** | sky_fires_on_draw |
| `AG:朝同一片天空迈进#4` | game-start draw is shuffled back | **unc** | — |
| `AG:绯红之魂#1` | [手] place on user's [场地] | **ok** | soul_places_and_loads_crystals |
| `AG:绯红之魂#2` | spend 1-5 x 500, place that many crystals | **ok** | soul_places_and_loads_crystals |
| `AG:绯红之魂#3` | [持续](1) [反击] on owner's spend/pay: -1 crystal, -1000 (min 0) | **ok** | soul_counter_reduces_a_payment,ix_soul_crystal_vs_rent |
| `AG:绯红之魂#4` | (1b) if it was a [支付], payee gains 500 | **unc** | — |
| `AG:绯红之魂#5` | (2) owner's own skill (2) use removes 1 crystal | **unc** | — |
| `AG:绯红之魂#6` | (3) empty -> discard | **ok** | soul_leaves_when_empty |
| `AG:刻入天穹傲岸的烈光#1` | [反击] when you pass a character | **ok** | glory_counter_on_passing_a_character |
| `AG:刻入天穹傲岸的烈光#2` | gain half of their dearest tile's base price | **ok** | glory_counter_on_passing_a_character |
| `AG:刻入天穹傲岸的烈光#3` | they gain half of yours | **ok** | glory_counter_on_passing_a_character |
| `AG:（巴）商店街的救世主#1` | [反击] when another mortgages a shop-street deed | **unc** | no test |
| `AG:（巴）商店街的救世主#2` | buy it at half the normal buyout | **unc** | — |
| `AG:（巴）商店街的救世主#3` | [反击] when you mortgage a shop-street deed | **unc** | — |
| `AG:（巴）商店街的救世主#4` | extra mortgage income and flip the deed back | **unc** | — |
| `AG:（绯玛丽）如果并非没问题#1` | [反击] when your roll < 6 | **ok** | himari_card_bumps_a_low_roll |
| `AG:（绯玛丽）如果并非没问题#2` | result +1 | **ok** | himari_card_bumps_a_low_roll |
| `AG:（鸫）微小的『能做到』的事#1` | choose one of (1)/(2) | **shl** | hagumi_card_is_exclusive |
| `AG:（鸫）微小的『能做到』的事#2` | (1) [反击] number-range effects settle at theoretical max/min | **unc** | ruleset.rs::hagumi_marks_replays_through_a_dice_dependent_prompt is harness-only |
| `AG:（鸫）微小的『能做到』的事#3` | (2) treat as playing @Tsugu ycm | **unc** | — |
| `skill:美竹兰:叛逆的红挑染#1` | every 3 turns w/o AG (2): +1 fire (init 1, cap 1) | **ok** | ran_skill_starts_with_one_fire |
| `skill:美竹兰:叛逆的红挑染#2` | pre-roll: spend 1 fire, move backward; no CiRCLE reward | **ok** | ran_skill_can_move_backward,m20b_ran_backwards_no_reward |
| `skill:青叶摩卡:我行我素#1` | every 3 turns w/o AG (2): +1 fire (init 1, cap 1) | **shl** | moca_skill_teleports_to_the_actor (cap only) |
| `skill:青叶摩卡:我行我素#2` | on other's skill/card play: spend 1 fire, teleport to their tile (no settle/passives) | **ok** | moca_skill_teleports_to_the_actor |
| `skill:青叶摩卡:我行我素#3` | 2-use CD per player's passives/cards | **unc** | — |
| `skill:宇田川巴:豚骨酱油拉面大姐#1` | every 3 turns w/o AG (2): +1 fire (init 1, cap 1) | **unc** | — |
| `skill:宇田川巴:豚骨酱油拉面大姐#2` | pre-move: spend 1 fire, shift start 10 toward 银河拉面馆 | **ok** | utsugi_skill_shifts_start_toward_ramen,m21_tomoe_shifts_start |
| `skill:宇田川巴:豚骨酱油拉面大姐#3` | overshoot refills 1 fire | **unc** | — |
| `skill:宇田川巴:豚骨酱油拉面大姐#4` | on the ramen shop: (2) banned; may stay 1 turn, once per lap | **unc** | — |
| `skill:上原绯玛丽:大家一起迈出新的一步#1` | every 3 turns w/o AG (2): +1 fire (init 1, cap 1) | **unc** | — |
| `skill:上原绯玛丽:大家一起迈出新的一步#2` | pre-move: spend 1 fire, odd/even-only tiles this move | **ok** | himari_skill_parity_move |
| `skill:羽泽鸫:伟大的平凡#1` | every 3 turns w/o AG (2): +1 fire (init 1, cap 1) | **unc** | — |
| `skill:羽泽鸫:伟大的平凡#2` | on main move: lose 1 fire to reverse; next turn double-roll backward | **ok** | tsuzushi_skill_reverses_a_move |
| `skill:羽泽鸫:伟大的平凡#3` | backward past CiRCLE gives no reward | **unc** | — |
| `skill:Afterglow:商店街的宠儿#1` | buy shop-street or value<=1200 -> auto free house | **ok** | band_free_house_on_cheap_buy,band_free_house_on_shop_street_buy,ix_band_house_then_sunset_demolish |

### CRYCHIC

| clause | meaning | status | tests |
|---|---|---|---|
| `CRYCHIC:想要成为人类#1` | place on field, declare X in 1-20, write it down | **ok** | want_to_be_human_places_and_declares_x |
| `CRYCHIC:想要成为人类#2` | move roll < X: +1 crystal on the card | **ok** | want_to_be_human_crystal_on_low_roll |
| `CRYCHIC:想要成为人类#3` | [自动] turn start, >=2 crystals: clear them, next move += 20-X | **shl** | want_to_be_human_places_and_declares_x (boost unasserted) |
| `CRYCHIC:想要成为人类#4` | if that move is hit by an abnormal effect: +2 crystals | **unc** | — |
| `CRYCHIC:春日影#1` | [特] on draw if assets >= 20000: may play from draw pile | **ok** | haruhikage_special_draw_gate |
| `CRYCHIC:春日影#2` | then draw until hand is 6; interrupted by a discard effect | **shl** | haruhikage_special_draw_gate |
| `CRYCHIC:春日影#3` | may be used as a [反击] | **ok** | haruhikage_plays_as_counter |
| `CRYCHIC:春日影#4` | use a Crychic character's skill | **shl** | haruhikage_plays_as_counter |
| `CRYCHIC:去唱卡拉ok吧#1` | roll up to 5 dice | **ok** | karaoke_rolls_multiple_dice |
| `CRYCHIC:去唱卡拉ok吧#2` | pick one as the move roll, move normally | **ok** | karaoke_rolls_multiple_dice,interaction_generic_encore_vs_karaoke |
| `CRYCHIC:想要抓住...#1` | gain 1 [停留] | **ok** | want_to_grab_gives_stay |
| `CRYCHIC:想要抓住...#2` | before next turn: first player who passes you -> you step 1 and settle before them | **unc** | interaction_ag_counter_vs_want_to_grab only checks the stay |
| `CRYCHIC:想要抓住...#3` | else at turn start: teleport to the nearest orthogonal tile and settle (main move) | **unc** | — |
| `CRYCHIC:优雅的呐喊#1` | [反击] when you draw outside your turn | **shl** | elegant_cry_is_a_counter |
| `CRYCHIC:优雅的呐喊#2` | at your next turn end: draw 1 | **unc** | — |
| `CRYCHIC:是我自己的问题#1` | [反击] after main move, before settle: step 1 away from the nearest player | **ign** | m28a_my_own_problem (DISCREPANCY: does not move) |
| `CRYCHIC:是我自己的问题#2` | (2) does not fire if hit by an abnormal move effect | **ign** | m28b_my_own_problem_after_forced_move (DISCREPANCY: still fires) |
| `CRYCHIC:如果能一直持续下去...#1` | [手] remove crystals from the band skill card, gain 2000+500*X | **ok** | if_only_it_lasted_hand_effect |
| `CRYCHIC:如果能一直持续下去...#2` | [持续] hand >= 7 -> this card goes to discard | **ign** | if_only_it_lasted_discards_at_hand_7 (DISCREPANCY: the hand>=7 → discard check never fires; the card stays on the field) |
| `CRYCHIC:一起演奏音乐的命运共同体#1` | [手] until your next turn start: on any tile's collection, move to the next tile owned by the player after you in order (no settle) | **shl** | destiny_together_plays |
| `CRYCHIC:一起演奏音乐的命运共同体#2` | if (1) did nothing: return this card to hand (band (3) does not fire) | **unc** | — |
| `CRYCHIC:主唱太拼命了#1` | [反击] when you would pay 5000+ in one go: cancel that payment | **ok** | vocalist_too_hard_is_a_counter,interaction_vocalist_counter_vs_big_payment |
| `CRYCHIC:初演大成功#1` | this turn your money cannot fall (auction and written-immune effects excepted) | **ok** | debut_success_blocks_money_decrease,interaction_debut_success_vs_rent |
| `CRYCHIC:初演大成功#2` | turn end: gain 1 [眩晕] and +1 crystal on the band skill card | **ok** | debut_success_stun_at_turn_end |
| `CRYCHIC:（睦）从没有觉得...#1` | choose (2) or (3) | **ok** | mutsumi_crychic_card_choice |
| `CRYCHIC:（睦）从没有觉得...#2` | (2) spend 3 band crystals (or all), run band (2), discard 1 | **shl** | mutsumi_crychic_card_choice |
| `CRYCHIC:（睦）从没有觉得...#3` | (3) [移除] this, add 表演的本能, reshuffle hand+discard into draw, draw 2 | **shl** | mutsumi_crychic_card_choice |
| `CRYCHIC:（睦）从没有觉得...#4` | (3b) this move starts at CiRCLE (no start-tile effect) | **unc** | — |
| `CRYCHIC:（灯）内心的呐喊#1` | reset 想要成为人类's X, gain 120 * |delta X| | **ok** | tomori_crychic_card_resets_x |
| `CRYCHIC:（灯）内心的呐喊#2` | band (2) does not remove it; shows and shuffles it into the draw pile | **unc** | — |
| `CRYCHIC:（灯）内心的呐喊#3` | if band is MyGO!!!!!: place on your earliest tile; within ±5 spend 1 fire to use MyGO Tomori (2) | **unc** | — |
| `CRYCHIC:（祥子）带领着大家#1` | turn start while stacked: may record those players and copy your main move onto them | **ign** | m27_sakiko_leads (DISCREPANCY: does not move P1) |
| `CRYCHIC:（祥子）带领着大家#2` | you settle first, then them in action order | **ign** | m27_sakiko_leads |
| `CRYCHIC:（祥子）带领着大家#3` | settlement payments during this are halved | **ign** | m27_sakiko_leads / saki_move_halves_settlement_payments family |
| `CRYCHIC:（soyo）回到曾经#1` | [特] on draw: play immediately, pick 1 or 2 | **ok** | soyo_crychic_card_special_on_draw |
| `CRYCHIC:（soyo）回到曾经#2` | 1: discard all hand, gain 500*count, draw 1, build a house free-ish, [移除] this | **shl** | soyo_crychic_card_special_on_draw |
| `CRYCHIC:（soyo）回到曾经#3` | 2: gain 1000 and add this to hand | **ok** | soyo_crychic_card_special_on_draw |
| `CRYCHIC:（soyo）回到曾经#4` | [手] gain 500 | **unc** | — |
| `CRYCHIC:（立希）即便比不上...#1` | [反击] after the move phase starts, before settle | **shl** | riki_crychic_card_is_counter |
| `CRYCHIC:（立希）即便比不上...#2` | re-roll until the face differs from every face this turn | **unc** | — |
| `skill:高松灯（CRYCHIC）:跌跌撞撞...#1` | pass CiRCLE: +1 fire (init 1, cap 1) | **ok** | tomori_skill_fire_on_circle |
| `skill:高松灯（CRYCHIC）:跌跌撞撞...#2` | any time in your turn: spend 1 fire to clear 1 [停留] | **unc** | — |
| `skill:高松灯（CRYCHIC）:跌跌撞撞...#3` | move roll <= 6: gain 1 [停留] and settle at turn end | **ok** | tomori_skill_low_roll_gives_stay |
| `skill:椎名立希（CRYCHIC）:克服劣等感#1` | if your move roll < another's last move roll (<=20) and you have not used a skill this turn, next turn you may re-roll once pre-move and keep either | **unc** | riki_skill_bound is binding-only |
| `skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？#1` | pass CiRCLE: +1 fire (init 1, cap 2) | **ok** | saki_crychic_skill_fire_cap |
| `skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？#2` | other's main-move end, before settle, if within ±5: they may pay 300 to step 1 toward you | **unc** | — |
| `skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？#3` | your main-move end: spend 1 fire to step 1 toward a player within ±5 | **unc** | — |
| `skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？#4` | if band is Ave Mujica: within 5 of a player = state 2, but only shuffling adds band crystals | **unc** | — |
| `skill:若叶睦（CRYCHIC）:精致的人偶#1` | pass CiRCLE: +1 fire (init 1, cap 1) | **ok** | mutsumi_crychic_skill_fire_cap |
| `skill:若叶睦（CRYCHIC）:精致的人偶#2` | game start: designate a player; pre-move spend 1 fire to copy their last move roll | **unc** | — |
| `skill:若叶睦（CRYCHIC）:精致的人偶#3` | designated player bankrupt: re-designate | **unc** | — |
| `skill:若叶睦（CRYCHIC）:精致的人偶#4` | band Ave Mujica: hand>=3 -> state 2, <3 -> state 1 (crystals only on shuffle) | **unc** | — |
| `skill:长崎素世（CRYCHIC）:雨中祈晴#1` | when another settles on your tile: may cancel that payment | **shl** | soyo_skill_cancels_payment_on_own_tile |
| `skill:长崎素世（CRYCHIC）:雨中祈晴#2` | your next main move becomes a teleport to that tile | **unc** | — |
| `skill:CRYCHIC:美好的往日幻影#1` | no hand limit; at hand >= 6: no gain/loss of money, no [手] plays, CiRCLE reward must be draw | **ign** | crychic_band_skill_hand_limit_none / crychic_band_skill_blocks_at_hand_6 / s05_crychic_band_six_cards (DISCREPANCY: money still moves) |
| `skill:CRYCHIC:美好的往日幻影#2` | turn end with empty draw+discard: remove this + all CRYCHIC cards, rebuild a 10-card pile with MyGO/Ave Mujica picks, gain that band skill, draw 2 | **unc** | crychic_band_skill_bound is binding-only |
| `skill:CRYCHIC:美好的往日幻影#3` | hand 4->5: +1 crystal (cap 10); on removal gain 500*X | **unc** | interaction_band_hand_limit_vs_draw touches the hand limit only |

### 通用 / CiRCLE

| clause | meaning | status | tests |
|---|---|---|---|
| `通用:@Tsugu ycm#1` | roll 3d10 | **ok** | tsugu_ge22_draws_a_card |
| `通用:@Tsugu ycm#2` | >=22: draw 1 | **ok** | tsugu_ge22_draws_a_card |
| `通用:@Tsugu ycm#3` | >=26: enter move phase, main move = teleport to bandori station, no settle | **ok** | tsugu_ge26_teleports_without_settle |
| `通用:@Tsugu ycm#4` | >=26: may buy any unowned purchasable tile | **ok** | tsugu_ge26_can_buy_any_unowned |
| `通用:@Tsugu ycm#5` | >=28: buy cost -1500 (min 0) | **ign** | tsugu_ge28_buy_discount_1500 is passing in rb_general but TEST-FINDINGS lists it as open; treat as covered-passing after recheck |
| `通用:@Tsugu ycm#6` | <26: choose +1000 or teleport-and-settle | **ok** | tsugu_lt26_choose_gain_1000,tsugu_lt26_choose_move_settles |
| `通用:登上武道馆#1` | X = ceil10(2000 / alive others) | **ok** | budokan_x_ceil10_per_alive_others |
| `通用:登上武道馆#2` | Y = alive others (recorded, unused downstream) | **shl** | budokan_targets_everyone_else |
| `通用:登上武道馆#3` | [指定] every other player | **ok** | budokan_targets_everyone_else,ix_declaration_vs_budokan |
| `通用:登上武道馆#4` | each pays user X | **ok** | budokan_x_ceil10_per_alive_others |
| `通用:GREAT#1` | [移除] this card | **ok** | great_removes_self_adds_perfect_gains_2000 |
| `通用:GREAT#2` | add a PERFECT to the draw pile and shuffle | **ok** | great_removes_self_adds_perfect_gains_2000 |
| `通用:GREAT#3` | gain 2000 | **ok** | great_removes_self_adds_perfect_gains_2000,ix_great_perfect_fever_chain |
| `通用:[衍生]PERFECT#1` | [移除] this card | **ok** | perfect_removes_self_adds_fever_gains_3000 |
| `通用:[衍生]PERFECT#2` | add a FEVER! to the discard pile | **ok** | perfect_removes_self_adds_fever_gains_3000 |
| `通用:[衍生]PERFECT#3` | gain 3000 | **ok** | perfect_removes_self_adds_fever_gains_3000 |
| `通用:[衍生]FEVER!#1` | place on user's [场地] | **ok** | fever_goes_to_owners_field |
| `通用:[衍生]FEVER!#2` | [持续](1) owner's [支付]/[获得] amounts +X | **ok** | fever_boosts_a_gain,fever_boosts_a_payment,fever_boosts_card_gains |
| `通用:[衍生]FEVER!#3` | X = 600 - 200 * (cards on owner's field) (may go negative) | **ok** | fever_x_counts_every_field_card,fever_x_falls_as_field_fills |
| `通用:[衍生]FEVER!#4` | (2) at owner's turn start: to user's discard | **ok** | fever_leaves_to_users_discard |
| `通用:10次招募（1回限定）#1` | [限] one copy per game | **ok** | recruit_allows_a_second_copy |
| `通用:10次招募（1回限定）#2` | spend 1500, draw 1 | **ok** | recruit_costs_1500_and_draws |
| `通用:该清CP了#1` | [特](1) [手] only if owner has <= 2 CP points | **ok** | cp_hand_is_not_gated_at_two_points |
| `通用:该清CP了#2` | [特](2) next 2 turn starts: spread 1 CP to an adjacent empty tile | **ok** | cp_spreads_to_adjacent_for_two_turn_starts |
| `通用:该清CP了#3` | [手] add 1 tile CP to an empty tile, +6 on-card CP to the card | **ok** | cp_places_one_tile_mark_and_six_on_card_cp |
| `通用:该清CP了#4` | settle on a CP tile: remove 1 tile CP + 1 on-card CP (the mark's `src` card), the settler gains 800 | **ok** | cp_settle_on_marked_tile_pays_800,cp_settle_spends_the_src_cards_on_card_cp_and_pays_the_settler |
| `通用:雨啊，快点来吧#1` | roll 2d2 as X | **ok** | rain_stays_x_players_including_user |
| `通用:雨啊，快点来吧#2` | [指定] X players, must include the user | **ok** | rain_user_is_always_targeted |
| `通用:雨啊，快点来吧#3` | each designated gains 1 [停留] | **ok** | rain_stays_x_players_including_user,ix_encore_vs_rain |
| `通用:网络链接异常#1` | [反击] before a [手] effect or event effect settles | **ok** | net_negates_untargeted_hand_effect,ix_net_vs_great |
| `通用:网络链接异常#2` | clause 1: [手] with [指定] -> cancel one designation | **ign** | net_cancels_one_target_not_all (DISCREPANCY: cancels every target) / c05_cancels_one_designation_of_four |
| `通用:网络链接异常#3` | clause 2: [手] without [指定] -> negate all effects | **ok** | net_negates_untargeted_hand_effect,c04_negates_an_untargeted_hand_effect,ix_netlink_vs_jennifer_target |
| `通用:网络链接异常#4` | clause 3: event card -> negate all effects | **unc** | — |
| `通用:尽力后的收获#1` | enter move phase; main move = any integer 1-6, then settle | **ok** | harvest_moves_chosen_distance_and_settles |
| `通用:CiRCLE THANKS PARTY!#1` | all others may spend 500; user spends 500 | **ok** | party_x1_grants_extra_turn,party_x2_roll_win_pays_out |
| `通用:CiRCLE THANKS PARTY!#2` | X = spenders + 1 (user counted) | **shl** | party_x1_grants_extra_turn |
| `通用:CiRCLE THANKS PARTY!#3` | X>=2: user rolls Xd20; >35 -> user +3000, other spenders +1500 | **ok** | party_x2_roll_win_pays_out,party_x2_roll_miss_pays_nobody |
| `通用:CiRCLE THANKS PARTY!#4` | X==1: user gets an extra turn after this one | **ok** | party_x1_grants_extra_turn |
| `通用:安可#1` | [反击] when the user is about to suffer any abnormal move effect | **ok** | encore_negates_abnormal_move_on_user,c06_encore_cancels_a_stay,m07_encore_vs_forced_move |
| `通用:安可#2` | negate that effect and everything it causes | **ok** | encore_negates_abnormal_move_on_user,c07_three_deep_encore_negated |
| `通用:[月岛麻里奈]今天也要加油工作喔#1` | [反击] when the user passes #1 and #1 is hit by another effect | **ok** | marina_card_counters_extra_effect_on_circle |
| `通用:[月岛麻里奈]今天也要加油工作喔#2` | the pass/settle on #1 goes through without the extra effect | **ok** | marina_card_counters_extra_effect_on_circle |
| `通用:[都筑诗船]Parking Space#1` | place on the Space tile; move its houses to your other tiles (max +1 each) | **ok** | parking_moves_space_houses_out |
| `通用:[都筑诗船]Parking Space#2` | [持续](1) that tile's settle becomes 'turn end: gain 1 [停留]' | **ok** | parking_replaces_settle_with_stay |
| `通用:[都筑诗船]Parking Space#3` | (2) players on that tile cannot use character or band skills | **ok** | parking_blocks_skills_on_the_tile,ix_parking_blocks_tsugu_skill |
| `skill:月岛麻里奈:礼物还有好多好多哟#1` | when another passes CiRCLE they may pay you 500 | **ok** | marina_skill_offered_on_pass |
| `skill:月岛麻里奈:礼物还有好多好多哟#2` | you roll 1d10; >=6 -> they gain 1200 | **ok** | marina_skill_roll_at_least_six_pays_1200,marina_skill_roll_under_six_pays_nothing |
| `skill:都筑诗船:尽力了吗#1` | pass CiRCLE: +1 fire (init 1, cap 2) | **ok** | tsugu_skill_gains_fire_on_pass,tsugu_skill_starts_with_one_fire |
| `skill:都筑诗船:尽力了吗#2` | main phase: spend 1 fire; you and one other get -1d6 / +1d6 on the next two move rolls | **ok** | tsugu_skill_spends_fire_and_pairs,tsugu_skill_first_roll_minus_d6 |
| `skill:都筑诗船:尽力了吗#3` | if that makes the move 0: settle in place | **ok** | tsugu_skill_zero_move_settles_in_place |
| `skill:都筑诗船:尽力了吗#4` | under that effect: settle costs except buying a deed are halved | **ok** | tsugu_skill_halves_settle_costs |
| `skill:都筑诗船:尽力了吗#5` | if another builds under that effect: you gain the house's full build cost | **unc** | — |
| `skill:都筑诗船:尽力了吗#6` | game start: own the Space tile | **ok** | tsugu_skill_starts_with_space |
| `skill:CiRCLE:后勤人员的努力#1` | deck may only contain 通用 cards | **unt** | circle_band_deck_rule_not_testable (harness builds its own decks) |
| `skill:CiRCLE:后勤人员的努力#2` | on your 通用 card settling: may discard 1 to double one number in its [手] effect (except 'add to deck') | **ign** | circle_band_doubles_a_number (DISCREPANCY: unported) |

### Hello, Happy World!

| clause | meaning | status | tests |
|---|---|---|---|
| `HHW:因为我一直相信着你#1` | gate: at least one other hand card | **ok** | believe_in_you_needs_a_second_hand_card |
| `HHW:因为我一直相信着你#2` | spend 800 | **ok** | believe_in_you_costs_800_and_swaps_hand_cards |
| `HHW:因为我一直相信着你#3` | reveal deck+hand; a player picks 1 of your hand into discard | **ok** | believe_in_you_costs_800_and_swaps_hand_cards |
| `HHW:因为我一直相信着你#4` | pick 2 from your deck into hand, reshuffle the draw pile | **ok** | believe_in_you_costs_800_and_swaps_hand_cards,interaction_believe_in_you_with_a_full_hand |
| `HHW:笑容大游行#1` | [反击] passing 弦卷集团: place this on it, end-point becomes the agent | **ign** | t13_smile_parade (DISCREPANCY: stack overflow) |
| `HHW:笑容大游行#2` | +3 crystals; each turn end remove 1; at 0 if still on 弦卷集团 -> discard | **ign** | t13_smile_parade |
| `HHW:笑容大游行#3` | [持续] after a settle: may move this to the end-point tile and clear its crystals | **unc** | — |
| `HHW:笑容大游行#4` | [持续] while on a tile that tile swaps places with 弦卷集团; after a settle there this goes to discard | **unc** | — |
| `HHW:出发！后台之旅！#1` | look at the top 2 | **ok** | backstage_looks_at_two_and_pays_1000_per_discarded |
| `HHW:出发！后台之旅！#2` | put 0-2 back in any order; the rest go to discard, +1000 each | **ok** | backstage_looks_at_two_and_pays_1000_per_discarded |
| `HHW:（薰）怪盗hello happy#1` | place on field (charge 3, decay 1) | **unc** | no test |
| `HHW:（薰）怪盗hello happy#2` | place a phantom-mark on another player's field | **unc** | — |
| `HHW:（薰）怪盗hello happy#3` | this turn you may force-stop on them when passing, and settle | **unc** | — |
| `HHW:（薰）怪盗hello happy#4` | while on field: anyone who can move and is on Kaoru's tile has main move = 1d2 direction + 1d10 distance | **unc** | — |
| `HHW:微笑巡逻队#1` | pay and build 1 house on your own tile | **ok** | smile_patrol_builds_a_house_and_may_build_another |
| `HHW:微笑巡逻队#2` | roll 3d20; on each matching tile index build 1 free house (if buildable) | **ok** | smile_patrol_builds_a_house_and_may_build_another,interaction_smile_patrol_and_owned_houses |
| `HHW:（kkr）前往笑容集结的地方！#1` | pay 10000 (counts as a buy), place this on CiRCLE | **ok** | t15_kkr_on_circle |
| `HHW:（kkr）前往笑容集结的地方！#2` | if another settles there: they pay the owner 6000, as a tile collection | **ok** | t15_kkr_on_circle |
| `HHW:黑衣人的补给#1` | [反击] passing CiRCLE: may play | **ok** | black_clothes_supply_is_a_counter_on_passing_circle |
| `HHW:黑衣人的补给#2` | place 1 crystal on 弦卷集团; while it has one, that tile has all of CiRCLE's effects | **ok** | t14_black_clothes_supply |
| `HHW:黑衣人的补给#3` | after you pass 弦卷集团: remove 1 crystal there | **ok** | t14_black_clothes_supply |
| `HHW:运动的天赋#1` | place on own field with 3 crystals; turn end lose 1; 0 -> discard | **ok** | athletic_talent_places_with_three_crystals,athletic_talent_burns_a_crystal_at_turn_end |
| `HHW:运动的天赋#2` | while on field: each roll gains money, starting 700, -100 each time, floor 100 | **unc** | — |
| `HHW:运动的天赋#3` | while on field: re-roll the move roll until >= 10 | **unc** | — |
| `HHW:Happy, Lucky, Smile, Yeah！#1` | roll 1d4 mod 4 -> up/down/left/right | **ok** | hlsy_teleports_to_the_farthest_tile_in_a_direction |
| `HHW:Happy, Lucky, Smile, Yeah！#2` | teleport to the farthest tile in that direction (main move) | **ok** | hlsy_teleports_to_the_farthest_tile_in_a_direction |
| `HHW:热气球演出#1` | roll 3d20 four times, record | **ok** | hot_air_balloon_teleports_to_a_chosen_roll |
| `HHW:热气球演出#2` | pick one, teleport to that tile index (main move) | **ok** | hot_air_balloon_teleports_to_a_chosen_roll |
| `HHW:爱心义演#1` | this turn, on a roll-move: +2 to the total per first pass over one of your tiles | **unc** | charity_live_halves_payments_this_turn asserts only the halving |
| `HHW:爱心义演#2` | this turn your payments to others are halved (ceil 10) | **ok** | charity_live_halves_payments_this_turn,interaction_charity_live_and_a_rent |
| `HHW:爱心义演#3` | if kkr's card is on CiRCLE, CiRCLE counts as hers | **ok** | t15_kkr_on_circle |
| `HHW:（花音）Wacha Mocha 啪嗒进行曲#1` | [场] Kanon gains 1 jellyfish mark per backward step | **unc** | no test |
| `HHW:（花音）Wacha Mocha 啪嗒进行曲#2` | at 9 marks: clear them, teleport to #4 or #30 (main move), shout, discard | **unc** | — |
| `HHW:（育美）#1` | [手] roll 4d20 twice; place a mark on one result tile | **ok** | t27_ikumi_marks |
| `HHW:（育美）#2` | passing the mark: may force-stop there, gain 2000, remove the mark | **ok** | t27_ikumi_marks |
| `HHW:（育美）#3` | if a chosen result > 60: +1000 and draw 1, no mark | **unc** | — |
| `HHW:（育美）#4` | [反击] at the end of a turn in which you received money outside your turn: +1 roll per gain this turn | **unc** | no gain-event observer (TEST-FINDINGS 3) |
| `HHW:（美咲）#1` | [反击] after a fire-pot move roll, before settle | **unc** | no test for the card (Misaki's skill is tested) |
| `HHW:（美咲）#2` | spend all fire pots, teleport to a player between start and end, settle (main move) | **unc** | — |
| `HHW:（美咲）#3` | then every other player on that tile and its 2 neighbours pays you 500 | **unc** | — |
| `HHW:梦幻的回礼#1` | [反击] when another is about to be charged by a non-yours tile | **shl** | dream_return_is_offered_when_another_player_is_charged |
| `HHW:梦幻的回礼#2` | pay one of your tiles on one other player | **unc** | — |
| `HHW:梦幻的回礼#3` | if the pay lands: run the band skill once as a double payment, +2 crystals recording both players | **unc** | — |
| `HHW:梦幻的回礼#4` | the charged player's bill drops by what you paid | **unc** | — |
| `skill:弦卷心:弦卷集团#1` | game start: +1000 | **ok** | kokoro_skill_1_starts_with_1000_extra |
| `skill:弦卷心:弦卷集团#2` | pass CiRCLE: +1500 extra | **ok** | kokoro_skill_2_gains_1500_extra_on_passing_circle,interaction_kokoro_circle_bonus_and_the_band_skill |
| `skill:松原花音:真正的迷子#1` | game start tile = #30 弦卷豪宅 | **ok** | kanon_skill_1_starts_on_the_mansion |
| `skill:松原花音:真正的迷子#2` | move phase: roll twice, move |a-b| | **unc** | no test asserts the subtraction |
| `skill:松原花音:真正的迷子#3` | if no CiRCLE reward in 3 turns: next move roll is 1d20+1d4 | **unc** | — |
| `skill:北泽育美:全垒打！#1` | each turn a croquette appears on 北泽精肉店 | **unc** | — (t29 does not exercise the per-turn generation) |
| `skill:北泽育美:全垒打！#2` | you passing a croquette: may take it to your field (cap 10) | **unc** | — (t29 exercises only the other-passer take) |
| `skill:北泽育美:全垒打！#3` | another passing: may take it and pay you 50*X (X = their croquettes) | **shl** | t29_croquettes (take asserted; 50*X amount is RULING: does X count the croquette just taken?) |
| `skill:北泽育美:全垒打！#4` | taking one may replace a mark not on a [持续] card | **unc** | — |
| `skill:北泽育美:全垒打！#5` | each croquette adds 1d2 to your move roll | **unc** | — |
| `skill:北泽育美:全垒打！#6` | you may hand a croquette to a character you pass | **unc** | — |
| `skill:北泽育美:全垒打！#7` | you: spend 4 croquettes for 1000; others with 4: auto-lose all and stay 1 turn | **unc** | — |
| `skill:奥泽美咲:另一个我#1` | pass 弦卷集团: +2 fire (init 0, cap 2) | **ok** | misaki_skill_1_gains_fire_pots_after_passing_the_tsuzumi_group |
| `skill:奥泽美咲:另一个我#2` | if CiRCLE was between your teleport start and end: next pass 弦卷集团 also counts as CiRCLE | **unc** | — |
| `skill:奥泽美咲:另一个我#3` | turn start: spend 1 fire, +1d10 to the move roll and the move is a [传送] | **unc** | — |
| `skill:奥泽美咲:另一个我#4` | after a fire-pot move roll, before settle: spend 1 fire for 1 [除外] that lifts at your next turn start | **unc** | — |
| `skill:濑田薰:梦幻的王子殿下#1` | pass or be passed: if no phantom-mark on field +1 fire (cap 7); else remove one mark | **unc** | no test |
| `skill:濑田薰:梦幻的王子殿下#2` | main phase: spend 7 fire, place 3 marks on the nearest player's field, main move starts at the smile-bus | **unc** | — |
| `skill:濑田薰:梦幻的王子殿下#3` | your non-teleport main move does not pass tiles owned by a player holding a mark | **unc** | — |
| `skill:Hello, Happy World!:传播笑容#1` | paying another's tile: may pay double; record that player, +1 crystal | **ok** | band_skill_1_offers_a_double_payment,interaction_kokoro_circle_bonus_and_the_band_skill |
| `skill:Hello, Happy World!:传播笑容#2` | a recorded player's tile collects: remove 1 crystal, record the collected amount (accumulating) | **unc** | — |
| `skill:Hello, Happy World!:传播笑容#3` | when you build: subtract the recorded amount from the cost, then zero it | **unc** | — |

### Morfonica

| clause | meaning | status | tests |
|---|---|---|---|
| `Mor:迷茫之蝶们的三全音#1` | [反击] when you are about to lose or pay money | **ok** | tritone_offered_when_about_to_pay |
| `Mor:迷茫之蝶们的三全音#2` | immediately gain that amount; place this on field with 3 crystals (3 turns) | **ok** | tritone_gains_amount_and_places_with_crystals |
| `Mor:迷茫之蝶们的三全音#3` | turn end remove 1 crystal; at 0 discard and pay back what it gained | **ok** | tritone_countdown_discards_and_pays_back |
| `Mor:迷茫之蝶们的三全音#4` | answers card-driven payments too | **ign** | c18_tritone_vs_card_payment (DISCREPANCY: does not answer) |
| `Mor:夏日合宿#1` | until your next turn start: only your own effects may designate you | **ok** | summer_plays_to_field_until_next_turn_then_draws,interaction_summer_shield_vs_foreign_targeting |
| `Mor:夏日合宿#2` | if it negated nothing when it ends: draw 1 | **ok** | summer_plays_to_field_until_next_turn_then_draws |
| `Mor:你的光芒将照亮前路#1` | gate: within 20 of 月之森 | **ok** | light_gate_within_20_of_tsukinomori |
| `Mor:你的光芒将照亮前路#2` | this move starts at 月之森 (no start-tile effect) | **ok** | light_move_starts_from_tsukinomori |
| `Mor:蝴蝶飞舞的星月夜#1` | pay X x 1000, place on field centre with X crystals; 0 crystals -> discard | **ok** | starry_pays_x_times_1000_and_places_x_crystals,starry_x_is_at_least_one,starry_no_crystals_removes_the_card,starry_empty_goes_to_discard |
| `Mor:蝴蝶飞舞的星月夜#2` | on field: you may abandon your first move roll and re-roll | **ok** | starry_owner_may_reroll_the_move_roll |
| `Mor:蝴蝶飞舞的星月夜#3` | when another's move roll is even: may spend 1000 and remove 1 crystal | **ign** | starry_even_roll_of_another_player_offers_crystal_removal (DISCREPANCY: never prompts) |
| `Mor:蝴蝶飞舞的星月夜#4` | owner passing CiRCLE: remove 1 crystal | **ok** | starry_circle_pass_removes_crystal_and_pays_1000 |
| `Mor:蝴蝶飞舞的星月夜#5` | each crystal removed: owner gains 1000 | **ok** | starry_circle_pass_removes_crystal_and_pays_1000 |
| `Mor:勇气展翅高飞之时#1` | roll 3d20 | **ok** | courage_sums_3d20_for_the_tile_number |
| `Mor:勇气展翅高飞之时#2` | that tile's owner pays you (price + houses)/2 | **ok** | courage_owner_pays_half_of_price_plus_houses |
| `Mor:勇气展翅高飞之时#3` | agent tile -> you gain 1000 | **ok** | courage_agent_tile_gives_1000 |
| `Mor:勇气展翅高飞之时#4` | unowned tile -> pays nothing | **ok** | courage_unowned_tile_pays_nothing |
| `Mor:（NNM）稍微努力了一下#1` | discard x character-marks from hand | **ign** | nnm_refused_without_character_marks / nnm_discard_marks_draw_x_or_gain_x_times_2000 (DISCREPANCY: dual On::Play) |
| `Mor:（NNM）稍微努力了一下#2` | (1) draw x (may exceed limit), turn end discard to 5 | **ign** | nnm_discard_marks_draw_x_or_gain_x_times_2000 |
| `Mor:（NNM）稍微努力了一下#3` | (2) gain x*2000 | **ign** | nnm_discard_marks_draw_x_or_gain_x_times_2000 |
| `Mor:（NNM）稍微努力了一下#4` | (3) place with x crystals; remove 1 to use skill (2) ignoring limits | **ign** | nnm_discard_marks_draw_x_or_gain_x_times_2000 |
| `Mor:（NNM）稍微努力了一下#5` | (4) crystals empty -> discard | **ign** | nnm_discard_marks_draw_x_or_gain_x_times_2000 |
| `Mor:（toko）#1` | swap one mortgaged deed of yours with a same-colour mortgaged deed of anyone | **ok** | toko_swaps_same_color_mortgaged_deeds_and_pays_difference |
| `Mor:（toko）#2` | the cheaper side pays the difference | **ok** | toko_swaps_same_color_mortgaged_deeds_and_pays_difference |
| `Mor:（toko）#3` | the deed you receive is redeemed for free | **unc** | — |
| `Mor:（小白）#1` | [反击] when you are about to pay another player | **ok** | xiaobai_turns_pay_into_lose_and_target_loses_half,c19_shiroko_vs_card_payment |
| `Mor:（小白）#2` | the pay becomes a loss of the same amount and the target also loses half | **ok** | xiaobai_turns_pay_into_lose_and_target_loses_half,s09_mujica_band_plus_shiroko |
| `Mor:（小白）#3` | answers card-driven payments too | **ok** | xiaobai_offered_on_card_driven_payments_too |
| `Mor:离心力，不为所动#1` | [反击] on the 2nd time you are targeted between your turns | **ok** | centrifuge_not_offered_on_the_first_targeting,centrifuge_offers_on_the_second_targeting,c09_centrifugal_on_the_second_targeting |
| `Mor:离心力，不为所动#2` | until your next turn start: negate everything aimed at you | **ok** | interaction_xuanzhan_and_centrifuge_answer_the_same_targeting |
| `Mor:高贵的微蓝#1` | gate: on a tile with deed value >= 2200 | **ok** | noble_gate_requires_deed_value_2200 |
| `Mor:高贵的微蓝#2` | if the tile is yours: settle immediately | **unc** | — |
| `Mor:高贵的微蓝#3` | place a mark; a marked tile cannot be designated | **ok** | noble_on_own_tile_places_a_no_target_mark,noble_on_foreign_tile_places_no_mark |
| `Mor:再次牵起手来#1` | [反击] when the player before you in order spends/pays > 0: place on your field and spend the same | **ok** | again_hand_counter_places_on_field_and_loses_equal,c17_holding_hands_again_cancels_payment |
| `Mor:再次牵起手来#2` | [持续] on a spend/pay: cancel that money change and go to discard | **ok** | again_persist_cancels_the_owners_next_payment,again_spent_card_goes_to_discard,interaction_again_cancels_toubudoukan_payment |
| `Mor:纯真振翅#1` | teleport 20 forward in the move direction (no settle) | **ok** | wing_teleports_20_without_settling |
| `Mor:纯真振翅#2` | then immediately make a move roll | **ign** | wing_then_rolls_the_movement_dice (DISCREPANCY: never rolls) |
| `Mor:秘密与青春的虹彩#1` | [反击] when you pay a junior or same-grade: halve the payment | **ok** | rainbow_halves_a_payment_to_a_same_grade |
| `Mor:秘密与青春的虹彩#2` | [反击] when a senior or same-grade pays you: multiply by 1.5 | **ign** | rainbow_senior_pays_you_at_1_5x (DISCREPANCY: settles at 0) |
| `Mor:（Rui）正论恶魔#1` | pay every player 100 (immune to modification) | **ok** | rui_pays_100_to_all_then_receives_300_from_all |
| `Mor:（Rui）正论恶魔#2` | then every player pays you 300 (modifiable) | **ok** | rui_pays_100_to_all_then_receives_300_from_all |
| `Mor:（Rui）正论恶魔#3` | if your skill fires during this card: set x to 5 | **ign** | rui_card_sets_x_to_5_when_the_skill_fires (DISCREPANCY: x stays 3) |
| `Mor:（筑紫）迷茫的庭园#1` | gain 100 | **ok** | tsukushi_gains_100_and_teleports_to_the_tile_before_a_player |
| `Mor:（筑紫）迷茫的庭园#2` | roll 1d6, teleport to the tile before that player in order (self -> +1) | **ok** | tsukushi_gains_100_and_teleports_to_the_tile_before_a_player,tsukushi_self_choice_advances_one_tile |
| `Mor:（筑紫）迷茫的庭园#3` | counts as the main move; settle is optional | **ok** | tsukushi_settle_is_optional,tsukushi_teleport_counts_as_the_main_move |
| `Mor:（筑紫）迷茫的庭园#4` | until your next turn start: you cannot be hit by abnormal moves | **unc** | — |
| `skill:仓田真白:向后全速前进#1` | [主动移动] move roll becomes 2d20 | **ok** | mashiro_main_move_is_2d20_backwards |
| `skill:仓田真白:向后全速前进#2` | move backwards | **ok** | mashiro_main_move_is_2d20_backwards,m20a_mashiro_backwards_past_circle |
| `skill:桐谷透子:天上天下，唯我独尊#1` | pass CiRCLE: +1 fire (init 1, cap 1) | **ok** | toko_fire_pot_cap_is_one |
| `skill:桐谷透子:天上天下，唯我独尊#2` | once per turn, management phase: spend 1 fire, designate one of your tiles; all other players within ±1 pay you ceil100(0.5*rent / n) | **ok** | toko_skill2_splits_half_the_rent_over_the_neighbours,toko_skill2_is_once_per_turn |
| `skill:广町七深:这是很普通的事吧？#1` | pass CiRCLE: +1 fire (init 2, cap 2) | **ok** | nana_fire_pot_cap_is_two |
| `skill:广町七深:这是很普通的事吧？#2` | when a player uses a skill: pay 1 fire to gain 1 of that character's marks (cap 1 each at 3+ players; 3-turn CD on passives) | **unc** | marks unobtainable (TEST-FINDINGS 4) |
| `skill:广町七深:这是很普通的事吧？#3` | spend a mark to use that character's skill, covering its costs | **unc** | — |
| `skill:八潮瑠唯:正论暴击#1` | if your (2) or character card is copied and the copy sets x to 0, they lose that (2) | **unc** | — |
| `skill:八潮瑠唯:正论暴击#2` | on a non-mortgage/redeem money change after all modifiers and != 0: roll 1d20; <=X -> x=0 and double the gain / skip the loss; >X -> x+=1 (x starts 0) | **ok** | rui_crit_x_starts_at_zero_and_increments_on_a_miss,rui_crit_fires_on_a_gain_as_well |
| `skill:二叶筑紫:交给班长吧#1` | on a gain or a draw: may redirect it to another player and record the amount (draw records 2200); once per settlement chain | **ok** | tsukushi_skill1_redirects_a_gain |
| `skill:二叶筑紫:交给班长吧#2` | when another gains/draws: may flip their X mark and take the action instead, capped at record+300 (draws need record >= 2200) | **unc** | — |
| `skill:二叶筑紫:交给班长吧#3` | when every other player has a flipped mark: remove all, gain 800 each (per-use -200 per mark, floor 200) | **unc** | — |
| `skill:Morfonica:振翅高飞的练习曲#1` | one-shot loss >= 2000: draw 1 | **ok** | morfonica_band_draws_on_a_2000_plus_loss |
| `skill:Morfonica:振翅高飞的练习曲#2` | CiRCLE reward 'gain money' cycles 1000 / 1500 / 2000 | **ok** | morfonica_band_circle_money_cycles_1000_1500_2000 |

### Ave Mujica

| clause | meaning | status | tests |
|---|---|---|---|
| `Mujica:（睦/mortis）表演的本能#1` | on play: copy the last card another player played (not cards placed on field) | **shl** | mortis_instinct_plays_without_error |
| `Mujica:（睦/mortis）表演的本能#2` | when copying a [反击] card it may be played as one at the right time | **unc** | — |
| `Mujica:欢迎来到ave mujica的世界#1` | choose one: (1) switch one player's state (no-op if they have no state 2) | **ok** | welcome_to_ave_mujica_switches_state (c27_welcome_state_switch still ign: DISCREPANCY play_phase) |
| `Mujica:欢迎来到ave mujica的世界#2` | (2) [反击] when another switches state: you and everyone who switched this turn switch once | **unc** | — |
| `Mujica:祥，移动#1` | force a player 3 tiles in a direction you choose, then settle | **ok** | saki_move_forces_three_tiles,saki_move_backward,interaction_saki_move_onto_cafe |
| `Mujica:祥，移动#2` | may target yourself before the roll instead of the main move | **unc** | — |
| `Mujica:祥，移动#3` | settlement payments during this are halved | **ign** | saki_move_halves_settlement_payments (DISCREPANCY: full price) |
| `Mujica:燃尽前的线香花火#1` | place on field with 2 crystals (cap 2) | **ok** | sparkler_places_with_two_crystals_and_grants_extra_turn |
| `Mujica:燃尽前的线香花火#2` | turn end: auto-remove 1 crystal and grant an extra turn | **ok** | sparkler_places_with_two_crystals_and_grants_extra_turn (l07_extra_turn_cascade ign: DISCREPANCY the last removal nets 1 [眩晕], want 2) |
| `Mujica:燃尽前的线香花火#3` | last crystal: discard and gain 2 [眩晕] | **ok** | sparkler_last_crystal_goes_to_owner_discard,sparkler_last_crystal_grants_stun |
| `Mujica:骰子已经掷下#1` | place on own field; this turn no other player may use [反击] from hand | **ok** | dice_cast_places_on_field,dice_cast_blocks_other_counters |
| `Mujica:骰子已经掷下#2` | turn end: to discard | **ok** | dice_cast_goes_to_discard_at_turn_end |
| `Mujica:骰子已经掷下#3` | this card can itself be countered | **ok** | c15_dice_cast_itself_countered |
| `Mujica:骰子已经掷下#4` | placed once (not twice) | **ign** | c14_dice_cast_shuts_windows (DISCREPANCY: placed twice, windows not shut) |
| `Mujica:无法将视线移开#1` | force every player who countered you this turn 1-4 tiles in a direction you choose, then settle | **ign** | c16_cannot_look_away_answers_a_counter (DISCREPANCY: no force) |
| `Mujica:无法将视线移开#2` | may be used as a [反击] right after a player counters you | **ign** | c16 / l04 (DISCREPANCY: err.play_phase) |
| `Mujica:无法将视线移开#3` | if in your turn you target yourself, it counts as your main move | **unc** | — |
| `Mujica:（初华）我，无畏悲伤#1` | gate: since your last CiRCLE pass, a [回忆地块] has settled | **ok** | hina_fearless_sadness_gate_blocks,hina_fearless_sadness_after_memory_tile |
| `Mujica:（初华）我，无畏悲伤#2` | then draw 1 or make this turn's rolls any value in 1-6 | **shl** | hina_fearless_sadness_after_memory_tile |
| `Mujica:（祥子）斩断留恋，忘却一切#1` | mortgage your dearest unmortgaged deed and discard 1 hand card | **ok** | saki_cut_ties_mortgages_and_teleports |
| `Mujica:（祥子）斩断留恋，忘却一切#2` | teleport to any purchasable or owned tile and settle (main move) | **ok** | saki_cut_ties_mortgages_and_teleports |
| `Mujica:（祥子）斩断留恋，忘却一切#3` | played from the draw pile: may skip the discard, or decline | **ok** | saki_cut_ties_card_ends_in_discard |
| `Mujica:心の雨#1` | every player within 20 ahead rolls 1d20 | **ok** | heart_rain_stays_within_20_tiles |
| `Mujica:心の雨#2` | <=12: 1 [停留]; <=3: 2 [停留] | **ok** | heart_rain_low_roll_gives_two_stay |
| `Mujica:心の雨#3` | if nobody got a stay: you gain 1 [眩晕] and 1000 | **ok** | heart_rain_fallback_stun_and_money,heart_rain_nobody_in_range_fallback |
| `Mujica:#J11#1` | place on own field with 2 crystals; turn end remove 1; 0 -> discard | **ok** | j11_places_with_two_crystals,j11_decays_one_crystal_per_turn |
| `Mujica:#J11#2` | any time you would lose money or be designated: may discard this and give 1 [眩晕] to you and anyone within ±1 | **unc** | — |
| `Mujica:人偶的箱庭#1` | every other player picks: move a move-roll (no settle) or pay you 20*distance-to-you | **ok** | dollhouse_others_choose_and_p0_moves_sum,m24_doll_box,m25_doll_box_cannot_move |
| `Mujica:人偶的箱庭#2` | a player who cannot move must pay | **ok** | m25_doll_box_cannot_move |
| `Mujica:人偶的箱庭#3` | you then move the sum of their move rolls (main move) | **ok** | dollhouse_others_choose_and_p0_moves_sum,m24_doll_box |
| `Mujica:会被骗着买水晶的人#1` | move one crystal from one card to another that can hold crystals | **ok** | crystal_move_from_band_to_skill_card,crystal_move_opens_source_and_target_prompts,crystal_move_no_crystals_is_noop |
| `Mujica:黑色生日#1` | each other player with money <= 1000 pays you 800 | **ok** | black_birthday_pays_by_money_bracket,black_birthday_boundary_1000 |
| `Mujica:黑色生日#2` | each other player with money > 1000 pays you 200 twice | **ok** | black_birthday_pays_by_money_bracket,interaction_black_birthday_money_stack |
| `Mujica:（喵梦）#1` | place on field | **ok** | nyamu_card_places_on_field |
| `Mujica:（喵梦）#2` | before flipping front->back: may lose 2 fire pots to stop the flip | **unc** | — |
| `Mujica:（喵梦）#3` | before flipping back->front: owner draws 1 | **unc** | — |
| `Mujica:（海铃）#1` | place on the field of the player after the user; at the user's turn start move it one seat forward | **ok** | umirin_card_places_on_next_player |
| `Mujica:（海铃）#2` | on play and at the user's turn start: take the band skill cards of every player holding this (no stacking) | **unc** | — |
| `Mujica:（海铃）#3` | when it returns to the user's field: at their turn end discard it and every band skill taken, draw 1 | **unc** | — |
| `skill:三角初华:Imprisoned XII#1` | state 1: move = 10+1d10 | **ok** | hina_skill_state1_move_is_10_plus_1d10,hina_skill_state1_move_replaces_d20 |
| `skill:三角初华:Imprisoned XII#2` | after a CiRCLE pass, if no [回忆地块] settle since the last one: may enter state 2 at next turn start | **unc** | — |
| `skill:三角初华:Imprisoned XII#3` | if that held: immediately after the CiRCLE-pass turn end you gain a new turn | **unc** | — |
| `skill:三角初华:Imprisoned XII#4` | state 2: fire cap 3, gain [不可阻挡] | **unc** | hina_skill_state2_prompt_at_turn_start only checks the prompt |
| `skill:三角初华:Imprisoned XII#5` | state 2: passing a player charges them 200; after each charge may +1d6 (max 4) | **unc** | — |
| `skill:若叶睦:天生的演员#1` | state 1: every 6 turns with no hand card played: draw 1 before turn start | **unc** | mutsumi_skill_bound is binding-only |
| `skill:若叶睦:天生的演员#2` | at turn start with hand == 5: may enter state 2 | **unc** | — |
| `skill:若叶睦:天生的演员#3` | state 2: take the last player-who-played-a-card's character card, cover yours, gain their initial fire; exit at hand <= 1 | **unc** | — |
| `skill:丰川祥子:请把你们的人生交给我#1` | state 1: passing a player: may absorb all their stays/stuns (move completes); +1500 per layer absorbed | **ign** | s08_sakiko_absorbs_statuses (DISCREPANCY: absorbs nothing) |
| `skill:丰川祥子:请把你们的人生交给我#2` | on taking stay/stun/exile: +1 fire (init 0, cap 3); at cap may enter state 2 at turn start | **ign** | saki_skill_bound_with_fire_cap is binding+cap only |
| `skill:丰川祥子:请把你们的人生交给我#3` | state 2: keep 1 hand card, discard the rest; each turn start auto-play the top of the draw pile | **unc** | — |
| `skill:八幡海铃:熟练的支援贝斯手#1` | state 1: others may pay you 400 at their turn start to half-price-settle one of your tiles on a neighbour; 2-turn re-charge CD | **unc** | umiri_skill_fire_cap is cap only |
| `skill:八幡海铃:熟练的支援贝斯手#2` | every 2 turns with no take: +1 fire (init 0, cap 4); at cap may enter state 2 | **unc** | — |
| `skill:八幡海铃:熟练的支援贝斯手#3` | state 2: spend X fire, mark X of their deeds; each pays 100+n*100 until state 2 ends; you split their rent; every 4 fire spent draw 1 | **unc** | — |
| `skill:祐天寺若麦:大主播喵梦亲#1` | state 1: on a payment to you +1 fire (init 0, cap 5) and the payment += 100*fire | **ok** | s10_numa_raises_payments |
| `skill:祐天寺若麦:大主播喵梦亲#2` | at fire cap: immediately enter state 2 | **unc** | nyamu_skill_fire_cap is cap only |
| `skill:祐天寺若麦:大主播喵梦亲#3` | state 2: flip every face-up hand/event card face-down; while state 2 face-down cards have no effect and are unaffected; flip back on exit | **unc** | — |
| `skill:Ave Mujica:假面之下的真实#1` | you have states 1 and 2; start in 1 | **ok** | ave_mujica_band_skill_bound |
| `skill:Ave Mujica:假面之下的真实#2` | state 2's 3rd turn start: +1 crystal | **ok** | ave_mujica_band_skill_crystal_on_state2_third_turn |
| `skill:Ave Mujica:假面之下的真实#3` | state 2: your [收取]/[支付] amounts are 1.5x (you only) | **shl** | s09_mujica_band_plus_shiroko |
| `skill:Ave Mujica:假面之下的真实#4` | state 2: may spend 1 crystal to halve one [支付] | **unc** | — |
| `skill:Ave Mujica:假面之下的真实#5` | state 2 with fire: refill to cap, lose 1 per turn end, exit at 0; exit loses all remaining fire | **unc** | — |

### MyGO!!!!!

| clause | meaning | status | tests |
|---|---|---|---|
| `MyGO:即使迷茫着#1` | [反击] when another's card affects you: place on your own field | **ok** | confused_counter_places_on_own_field,confused_counter_not_for_own_card |
| `MyGO:即使迷茫着#2` | [持续] main phase: may discard this, enter move phase, main move = your hand count | **ok** | confused_持续_moves_by_hand_count |
| `MyGO:（灯）不再迷茫#1` | gain 2 stays only removable by natural decay; place on field; discard all hand; +X+1 crystals (X = discarded) | **ok** | light_places_with_stay_and_x_plus_1_crystals,light_zero_hand_gives_one_crystal |
| `MyGO:（灯）不再迷茫#2` | on a skill use: may remove 1 crystal instead of the fire cost | **ign** | light_crystal_pays_skill_fire_cost (DISCREPANCY: does not spend the crystal) |
| `MyGO:（灯）不再迷茫#3` | crystals empty -> [移除] this card | **ign** | light_removed_when_crystals_run_out (DISCREPANCY: never empties) |
| `MyGO:轮符雨#1` | gain 1 [停留] | **ok** | rain_gives_one_stay |
| `MyGO:轮符雨#2` | turn end: one extra [触发结算] | **ok** | rain_extra_settle_at_turn_end |
| `MyGO:壱雫空#1` | clear every [停留] and [眩晕] on the field | **ign** | clear_removes_all_stay_and_stun / clear_charges_every_player_per_type / clear_user_effect_gives_extra (partially wrong) |
| `MyGO:壱雫空#2` | every player pays the user 1000 per cleared effect type | **ign** | clear_charges_every_player_per_type (DISCREPANCY: only affected players) |
| `MyGO:壱雫空#3` | if the user's own effects were cleared: +1000 extra per type | **ign** | clear_user_effect_gives_extra (DISCREPANCY: not paid) |
| `MyGO:壱雫空#4` | playable while stunned | **ok** | clear_playable_while_stunned |
| `MyGO:无路矢#1` | designate another player's tile; gain 2 [除外] | **ok** | no_road_gives_two_exile_aimed_at_player_tile |
| `MyGO:无路矢#2` | when exile hits 0: teleport to that tile (main move) | **ok** | no_road_teleports_when_exile_wears_off |
| `MyGO:无路矢#3` | during exile, tile income you would get goes to the designated player | **ok** | no_road_income_goes_to_the_designated_player |
| `MyGO:羽丘的不可思议女孩#1` | gate: on your own tile | **ok** | haneoka_below_10_is_ineffective |
| `MyGO:羽丘的不可思议女孩#2` | roll 1d20; >10: free house on this tile | **ok** | haneoka_over_10_builds_free_house |
| `MyGO:羽丘的不可思议女孩#3` | >15: also draw 1 | **ok** | haneoka_over_15_also_draws,haneoka_15_does_not_draw |
| `MyGO:羽丘的不可思议女孩#4` | >20: place on own field; later may discard to cancel any one payment | **ok** | haneoka_at_20_does_not_place (at 20 does not place; >20 untested) |
| `MyGO:羽丘的不可思议女孩#5` | strictly <10: to discard, card did not take effect | **ok** | haneoka_below_10_is_ineffective |
| `MyGO:[千早爱音]Anon Tokyo#1` | [限] gate: you are on a purchasable tile | **ok** | anon_gate_requires_purchasable_tile |
| `MyGO:[千早爱音]Anon Tokyo#2` | [指定] an adjacent purchasable tile | **ok** | anon_only_adjacent_purchasable_offered |
| `MyGO:[千早爱音]Anon Tokyo#3` | if you own both: spend half the higher price, place 1 link crystal (cap 1) | **ok** | anon_both_owned_pays_half_of_higher_price,anon_both_owned_places_link_marks,anon_link_is_capped_at_one |
| `MyGO:[千早爱音]Anon Tokyo#4` | linked tiles: settling one charges half the partner's rent extra | **ok** | anon_link_charges_half_of_linked_tile_rent,t21/t01 family |
| `MyGO:[千早爱音]Anon Tokyo#5` | if you do not own both: enter move phase, main move = 1 step to the designated tile, settle | **ok** | anon_not_both_owned_moves_one_and_settles |
| `MyGO:（soyo）混合的颜色#1` | place on a tile you own a deed for; that tile gains every colour | **ok** | soyo_mixed_colors_attaches_to_owned_tile |
| `MyGO:（soyo）混合的颜色#2` | it cannot be built on via another colour's agent | **unc** | — |
| `MyGO:（soyo）混合的颜色#3` | charging via another colour's agent from that tile halves again on top of the agent half | **ok** | soyo_mixed_colors_halves_agent_charge_again |
| `MyGO:（立希）想认真去做#1` | every player holding a [停留] settles on a tile within ±2 that you choose | **ok** | rikki_settle_quarter_pay_exact,rikki_only_stay_holders_settle |
| `MyGO:（立希）想认真去做#2` | all [支付] from that settle are a quarter of the normal price | **ok** | rikki_settle_quarter_pay_exact |
| `MyGO:（乐奈）有趣的女人#1` | place on the current tile | **ok** | rana_fun_woman_sits_on_current_tile |
| `MyGO:（乐奈）有趣的女人#2` | someone passes without settling: +1 crystal | **ok** | rana_fun_woman_gains_crystal_on_pass |
| `MyGO:（乐奈）有趣的女人#3` | at >=5 crystals: the next non-you passer picks 'lose a 抹茶芭菲' or force-stop and settle | **ok** | t18_rana_interesting_woman |
| `MyGO:（乐奈）有趣的女人#4` | if force-stopped: shuffle this into the discard pile | **ok** | t18_rana_interesting_woman |
| `MyGO:（乐奈）有趣的女人#5` | settlements caused by this card pay half | **ok** | t18_rana_interesting_woman |
| `MyGO:那天的雨#1` | place on own field with 5 crystals | **ok** | rain_day_places_five_crystals |
| `MyGO:那天的雨#2` | on play and each turn start: roll 1d10, the nth non-东京外 agent's colour zone (and its neighbours) gains 1 [停留] | **ok** | rain_day_d10_stays_the_matching_zone,rain_day_adjacent_tiles_also_stay |
| `MyGO:那天的雨#3` | face > 10 is fixed to the high-residential colour | **unc** | — |
| `MyGO:那天的雨#4` | turn end: remove 1 crystal; all gone -> discard | **ok** | rain_day_drains_one_crystal_per_turn_end |
| `MyGO:若能再次交汇#1` | [反击] after a move roll, if a passable player is on the field | **ok** | meet_again_rerolls_until_passing_a_player |
| `MyGO:若能再次交汇#2` | re-roll until you pass the next player (cap: 20 past the original end) | **ign** | m06_reunion_20_tile_cap / m06b (DISCREPANCY: world_mut during routine) |
| `MyGO:哪怕这旅程没有终点#1` | [手] place on current tile with 4 crystals | **ok** | journey_places_on_tile_with_four_crystals |
| `MyGO:哪怕这旅程没有终点#2` | turn end: if the main move was > 6, lose 1 crystal; 0 -> discard | **ok** | journey_drains_when_move_exceeds_six,journey_no_drain_when_move_is_six_or_less |
| `MyGO:哪怕这旅程没有终点#3` | [持续] on a settle: gain 60*X, X = tiles passed this move | **ok** | journey_settle_gains_x_times_60 |
| `MyGO:难以复刻的奇迹#1` | [手] place on field, move every band-skill crystal onto it | **ok** | miracle_transfers_band_crystals_on_play |
| `MyGO:难以复刻的奇迹#2` | when building: may remove 1 crystal instead of paying | **unc** | miracle_transfers_band_crystals_on_play covers the transfer only |
| `MyGO:难以复刻的奇迹#3` | [持续] after passing every player once: gain the last-3-digits of the player whose crystal count is closest, then discard | **unc** | — |
| `MyGO:难以复刻的奇迹#4` | if it enters the discard with crystals still on it: it never took effect | **ok** | miracle_ineffective_if_discarded_with_crystals |
| `MyGO:普通与理所当然#1` | [反击] after suffering an abnormal move effect | **ok** | m18_plain_and_ordinary,normal_ordinary_copies_last_main_move_steps |
| `MyGO:普通与理所当然#2` | your next main move distance = your last non-teleport main move distance | **ok** | m18_plain_and_ordinary |
| `skill:高松灯:诗超绊#1` | pass any RiNG: +1 fire (init 4, cap 4) | **ok** | tomori_fire_cap_is_four,tomori_initial_fire_is_four,tomori_fire_on_landing_on_ring,tomori_fire_on_passing_through_ring |
| `skill:高松灯:诗超绊#2` | when another's end is within ±5: spend 4 fire, before their settle teleport them to you, then 1d20 <=6 -> 1 [停留] and settle at your next turn end; payments under this are halved | **unc** | — |
| `skill:高松灯:诗超绊#3` | when your end is within ±5 of another: spend 1 fire to teleport to them before settling | **unc** | — |
| `skill:千早爱音:重新开始#1` | pass CiRCLE: +1 fire (init 2, cap 3) | **ok** | anon_char_fire_cap_is_three,anon_char_initial_fire_is_two,anon_char_fire_on_passing_circle |
| `skill:千早爱音:重新开始#2` | after a [停留]/[传送] effect resolves on you: spend 2 fire, draw 1 | **ok** | anon_char_draws_after_abnormal_resolves |
| `skill:长崎素世:通透的颜色#1` | pass CiRCLE: +1 fire (init 1, cap 1) | **ok** | soyo_char_fire_cap_is_one,soyo_char_initial_fire_is_one |
| `skill:长崎素世:通透的颜色#2` | after a move roll: spend 1 fire; your end counts as the matching agent tile until your next turn start | **unc** | — |
| `skill:长崎素世:通透的颜色#3` | if that saved you >= 1500 in one go: your next CiRCLE pass gives no fire | **unc** | — |
| `skill:椎名立希:决定练习日的会议#1` | whenever a player gains 1 [停留]: +1 fire (init 0, cap 5) | **ok** | taki_fire_cap_is_five_and_initial_zero,taki_fire_when_a_player_gains_stay |
| `skill:椎名立希:决定练习日的会议#2` | management phase: spend 1 fire, +1 crystal on any card on your field (once per turn) | **ok** | taki_skill_adds_crystal_once_per_turn |
| `skill:要乐奈:投币式停车场的猫#1` | pass CiRCLE: +1 fire (init 3, cap 3) | **ok** | rana_cat_fire_cap_is_three,rana_cat_initial_fire_is_three |
| `skill:要乐奈:投币式停车场的猫#2` | spend 3 fire to teleport to Space instead of the move; no deed purchase there | **ign** | rana_cat_space_teleport_spends_three_fire (DISCREPANCY: piece does not move) |
| `skill:要乐奈:投币式停车场的猫#3` | settling on Space: if another owns it, skip the payment and draw 1; if you own it, may instead add a 抹茶芭菲 (+500 rent each) | **unc** | — |
| `skill:要乐奈:投币式停车场的猫#4` | stacking with a player: you gain 800 of theirs and give them a 抹茶芭菲 | **unc** | — |
| `skill:要乐奈:投币式停车场的猫#5` | holding a 抹茶芭菲: passing any RiNG may spend it for 400 | **unc** | — |
| `skill:MyGO!!!!!:迷途之星#1` | game start: roll 3d20, that is your start tile | **ok** | band_start_tile_is_the_opening_3d20 |
| `skill:MyGO!!!!!:迷途之星#2` | move roll >= 16: +1 crystal (cap 1) | **ok** | band_roll_16_plus_adds_crystal,band_roll_below_16_adds_no_crystal,band_roll_gain_is_capped_at_one |
| `skill:MyGO!!!!!:迷途之星#3` | your turn, before the move roll: may move 1 instead, spending 1 crystal | **ok** | band_move_one_for_a_crystal,m22_mygo_band_move1 (ignored: replacement does not fire) |
| `skill:MyGO!!!!!:迷途之星#4` | a card entering the discard without taking effect: +1 crystal (may exceed cap by 2) | **ok** | band_ineffective_card_adds_over_cap_crystal |
| `skill:MyGO!!!!!:迷途之星#5` | your turn: spend 2 crystals to draw 1 | **ign** | band_draw_for_two_crystals (DISCREPANCY: unreachable second On::Play) |

### Pastel*Palettes

| clause | meaning | status | tests |
|---|---|---|---|
| `PP:再次闪耀#1` | [特] this card is unaffected by anyone but its owner | **ok** | ix_shine_immunity_flag |
| `PP:再次闪耀#2` | [手] place on user's [场地] | **ok** | shanyao_placed_on_field,shanyao_hand_play_places |
| `PP:再次闪耀#3` | [持续](1) [反击] when the owner spends/pays and cannot afford: record an unrecorded colour of a same-colour deed, gain (deed price)/5 and 1 face-up fan | **ok** | shanyao_counter_on_short_payment |
| `PP:再次闪耀#4` | (2) [共鸣] record a colour, owner gains 1 face-up fan | **unc** | — |
| `PP:同一个梦想#1` | add 3 crystals to your PP band card | **unc** | dream_steps_without_echo only covers fan flips |
| `PP:同一个梦想#2` | flip every face-up fan down, gain 100 * (flipped count) | **ok** | dream_steps_without_echo |
| `PP:同一个梦想#3` | flip every face-down fan up; with [共鸣] this hits every PP character | **ok** | dream_echo_flips_all_pp_characters |
| `PP:初次演出事故#1` | place on user's [场地], add 1 重叠的声音 to the draw pile, shuffle, draw 1 | **ok** | accident_places_shuffles_overlap_draws |
| `PP:初次演出事故#2` | [持续] owner cannot use any PP character's (2) skill | **ign** | accident_blocks_pp_skill_2 (DISCREPANCY: skillBlock not seen by the hook) |
| `PP:[衍生]重叠的声音#1` | [特] entering the owner's discard: [移除] their 初次演出事故 | **ok** | overlap_play_removes_accident |
| `PP:[衍生]重叠的声音#2` | [手] [移除] this and the user's 初次演出事故 | **ok** | overlap_play_removes_accident |
| `PP:[衍生]重叠的声音#3` | enter move phase; main move = teleport to bandori station, no settle | **ok** | overlap_play_teleports_and_turn_end |
| `PP:[衍生]重叠的声音#4` | turn end: 1 [停留] + 1 face-up fan, add a 明天见 to hand | **ok** | overlap_play_teleports_and_turn_end |
| `PP:[衍生]明天见#1` | [特](1) only if face-up fans <= face-down fans | **ok** | seeyou_gate_requires_up_le_down |
| `PP:[衍生]明天见#2` | (2) [共鸣] ignores (1) | **ok** | seeyou_echo_ignores_gate |
| `PP:[衍生]明天见#3` | [手] flip face-down fans up, count = your total fans | **ok** | seeyou_flips_fan_count_reverse_positive |
| `PP:不要背负期待#1` | [特] unaffected by any other effect | **ok** | ix_shine_immunity_flag / c26_immunity_vs_crystal_mover (also covers immunity) |
| `PP:不要背负期待#2` | [手] place on user's [场地], add a 共鸣 to the deck, draw 1 | **ok** | expect_hand_places_adds_resonance_draws |
| `PP:不要背负期待#3` | [持续](1) non-turn-start rolls -2 (move roll floor 0; 0 = no settle); hand limit -1 | **ok** | expect_non_turn_start_roll_minus_2,m04_minus_two_plus_yolo,m04b_minus_two_clamped_to_zero,expect_reduces_hand_limit |
| `PP:不要背负期待#4` | (2) on place and on each reshuffle: for 2 instances, pays +100 and gains -100 (floor 0) | **ok** | expect_pay_plus_100 |
| `PP:[衍生]共鸣#1` | [特] when the owner's card may fire [共鸣]: discard this, fire it, +2 crystals on the PP band card | **ok** | ix_echo_chain_adds_crystals |
| `PP:有你与我在这里共度#1` | X = 5 | **ok** | together_x_is_5 |
| `PP:有你与我在这里共度#2` | if face-up < face-down: X += ceil((down-up)/2) | **ok** | together_x_bonus_when_down_exceeds_up |
| `PP:有你与我在这里共度#3` | with [共鸣]: X -= 5 and draw 1 | **ok** | together_echo_x_minus_5_and_draw |
| `PP:有你与我在这里共度#4` | flip X face-down fans up | **ok** | together_x_is_5 |
| `PP:梦在前方，结彩当下#1` | [特] auto-place at game start; start hand -1 | **ok** | dream_ahead_autoplaced_at_game_start,dream_ahead_reduces_start_hand,dream_ahead_reduces_hand_limit |
| `PP:梦在前方，结彩当下#2` | [持续](1) each draw +1 crystal (cap 5); over-cap crystals each +1 to X (X starts 0) | **ok** | dream_ahead_crystal_per_draw_cap_5 |
| `PP:梦在前方，结彩当下#3` | (2) owner cannot build; hand limit -1 | **ok** | dream_ahead_no_build,dream_ahead_reduces_hand_limit |
| `PP:梦在前方，结彩当下#4` | (3) another settling on your tile pays you fans*X + MIN(X*30, 300) extra | **ok** | dream_ahead_extra_rent |
| `PP:梦在前方，结彩当下#5` | (4) [共鸣][反击] at your turn end with an unowned purchasable within ±3: spend 3 crystals, buy any such tile | **unc** | — |
| `PP:TITLE IDOL#1` | +2 crystals on your PP band card | **ok** | title_idol_adds_1_to_crystal_cards |
| `PP:TITLE IDOL#2` | +1 crystal on every card of yours that mentions crystals (2 with [共鸣]) | **ok** | title_idol_adds_1_to_crystal_cards,title_idol_echo_adds_2_to_crystal_cards |
| `PP:可爱又强壮的花朵#1` | [指定] one of your tiles; place this on it, +2 crystals on the PP band card | **ok** | flower_places_on_chosen_tile |
| `PP:可爱又强壮的花朵#2` | [持续] user passing this tile: force-stop, settle | **ok** | flower_pass_forces_stop_and_discards |
| `PP:可爱又强壮的花朵#3` | with [共鸣]: gain 1500 | **ok** | flower_echo_gives_1500 |
| `PP:可爱又强壮的花朵#4` | then this goes to the user's discard | **ok** | flower_pass_forces_stop_and_discards |
| `PP:找回珍妮弗#1` | [手] place on another player's [场地] | **ok** | jennifer_places_on_another_player |
| `PP:找回珍妮弗#2` | [持续] owner passing 偶像经纪公司: user pays owner 400 | **ok** | jennifer_pass_agency_pays_and_grants_ranger |
| `PP:找回珍妮弗#3` | user gains 1 face-up fan; owner flips 1 fan up | **ok** | jennifer_pass_agency_pays_and_grants_ranger |
| `PP:找回珍妮弗#4` | [移除] this, add a 魔法战队PastelRanger to the user's hand | **ok** | jennifer_pass_agency_pays_and_grants_ranger |
| `PP:[衍生]魔法战队Pastel✽Ranger#1` | [特] only if a tile you own a house on is within 30-fans | **ok** | rangers_gate_needs_house_within_range |
| `PP:[衍生]魔法战队Pastel✽Ranger#2` | step by cards you own on field (count +1 with [共鸣]) | **ok** | rangers_steps_by_card_count |
| `PP:[衍生]魔法战队Pastel✽Ranger#3` | >=1: gain 500 | **ok** | rangers_steps_by_card_count |
| `PP:[衍生]魔法战队Pastel✽Ranger#4` | >=3: +3 crystals on the PP band card | **ok** | rangers_steps_by_card_count |
| `PP:[衍生]魔法战队Pastel✽Ranger#5` | >=4: spend 4 band crystals, draw 1 | **ok** | rangers_steps_by_card_count |
| `PP:[衍生]魔法战队Pastel✽Ranger#6` | >=5: gain 1 layer of 'lose 2000, next build -2000 (overflow), drop 1 layer' | **unc** | — |
| `PP:[衍生]魔法战队Pastel✽Ranger#7` | >=6: draw 1 | **ok** | rangers_steps_by_card_count |
| `PP:练习生解密指南#1` | +3 crystals on the PP band card; place this on user's field | **ok** | guide_play_sets_crystals |
| `PP:练习生解密指南#2` | place fans/3 crystals on this card | **ok** | guide_play_sets_crystals |
| `PP:练习生解密指南#3` | reveal the draw pile, add a mono/duo mark per colour (2 marks and spend 500 with [共鸣]) | **unc** | — |
| `PP:练习生解密指南#4` | [持续](1) hand limit -1 | **ok** | guide_hand_limit_and_turn_end_crystal,expect_reduces_hand_limit |
| `PP:练习生解密指南#5` | (2) turn end: +1 crystal | **ok** | guide_hand_limit_and_turn_end_crystal |
| `PP:练习生解密指南#6` | (3) at >=5 crystals: each mono mark = 1 layer of 'next build -1000'; each duo mark = flip all your fans up; then discard | **unc** | — |
| `PP:[丸山彩]憧憬的前方#1` | [手] place on user's field, take 1 card from the discard into hand | **shl** | aya_exclusive_places_and_recycles |
| `PP:[丸山彩]憧憬的前方#2` | [持续](1) if you have the least money: your spends/pays drop by X (floor 0); X=100, +100 at >=10 fans | **unc** | TEST-FINDINGS: exclusive does nothing |
| `PP:[丸山彩]憧憬的前方#3` | (2) [共鸣][反击] on a spend/pay: -1500 (floor 0) | **unc** | — |
| `PP:[大和麻弥]可能性为∞#1` | [特] on viewing the deck and seeing this: reveal and place on your field first | **unc** | no test plays the card |
| `PP:[大和麻弥]可能性为∞#2` | [持续](1) after a draw: +1 crystal, or at >=4 move them to the PP band card | **unc** | — |
| `PP:[大和麻弥]可能性为∞#3` | (2) [共鸣] swap your draw and discard piles | **unc** | — |
| `PP:[白鹭千圣]微笑的铁假面#1` | [手] place on user's field; others [分摊] 2000 to the user | **shl** | chiasa_exclusive_places_and_splits_2000 |
| `PP:[白鹭千圣]微笑的铁假面#2` | [持续](1) on the owner's discard reshuffle: others [分摊] 500 | **unc** | — |
| `PP:[白鹭千圣]微笑的铁假面#3` | (2) [共鸣] others [分摊] 1500 | **unc** | — |
| `PP:[冰川日菜]会发出怎样的声音呢？#1` | [特](1) may stand in for Ayaka/Maya/Chisao/Eve's exclusive when their condition holds | **unc** | no test plays the card |
| `PP:[冰川日菜]会发出怎样的声音呢？#2` | (2) [共鸣] while on field, change the effect of the exclusive it stands in for | **unc** | — |
| `PP:[若宫伊芙]属于我的武士道！#1` | [手] place on user's field; roll 12d4, gain 60*sum | **ok** | eve_exclusive_rolls_12d4_times_60 |
| `PP:[若宫伊芙]属于我的武士道！#2` | [持续](1) after a draw: +1 crystal (cap 3) | **ok** | eve_exclusive_crystal_per_draw |
| `PP:[若宫伊芙]属于我的武士道！#3` | (2) before a settle, if this has crystals and the tile belongs to another: spend 1, owner rolls 1d6, user rolls 3d4 | **unc** | — |
| `PP:[若宫伊芙]属于我的武士道！#4` | (2b) lower roller pays the higher (houses+1)*100; tie = nobody; with [共鸣] the pay is |delta|*50 | **unc** | — |
| `skill:丸山彩:With~#1` | game start: 5 face-up fans; every non-PP player gains Aya's (2) | **ok** | skill_1_grants_5_positive_fans_at_game_start,band_skill_1_grants_reverse_fan_to_non_pp |
| `skill:丸山彩:With~#2` | on your pay: may flip Y fans down, the pay drops Y*100 (floor 0); PP chars also flip Y other-band fans up, others flip 1 fan up on every PP char | **ok** | aya_skill_2_discount_and_flip,ix_aya2_discount_vs_rent |
| `skill:若宫伊芙:天下统一#1` | game start: 5 face-up fans; every non-PP player gains Eve's (2) | **ok** | skill_1_grants_5_positive_fans_at_game_start |
| `skill:若宫伊芙:天下统一#2` | before any roll: may flip Y fans down and add Yd4 to the roll; same fan-flip back-side | **ok** | eve_skill_2_adds_yd4,m03_eve_then_yolo (ignored: dice accounting) |
| `skill:白鹭千圣:保持坦率的你#1` | game start: 5 face-up fans; every non-PP player gains Chisao's (2) | **ok** | skill_1_grants_5_positive_fans_at_game_start |
| `skill:白鹭千圣:保持坦率的你#2` | on your gain: may flip Y fans down, the gain +Y*100; same fan-flip back-side | **ok** | chiasa_skill_2_income_boost |
| `skill:大和麻弥:朝阳照耀的片刻#1` | game start: 5 face-up fans; every non-PP player gains Maya's (2) | **ok** | skill_1_grants_5_positive_fans_at_game_start |
| `skill:大和麻弥:朝阳照耀的片刻#2` | on your draw: may flip Y fans down, look at Y+1 and keep 1 (counts as a draw), rest back; same fan-flip back-side | **ok** | maya_skill_2_watch_and_pick |
| `skill:冰川日菜:日菜抽中的大奖#1` | game start: 5 face-up fans; every non-PP player gains Hina's (2) | **ok** | skill_1_grants_5_positive_fans_at_game_start |
| `skill:冰川日菜:日菜抽中的大奖#2` | each turn start: roll 1d4, borrow that character's (2) until your next turn start | **ign** | hina_skill_2_borrows_skill (DISCREPANCY: borrowed stays -1) |
| `skill:Pastel✽Palettes:与偶像一起#1` | game start: every non-PP character gains 1 face-down fan (no stacking) | **ok** | band_skill_1_grants_reverse_fan_to_non_pp |
| `skill:Pastel✽Palettes:与偶像一起#2` | if a 'flip X fans up' hits you and X > your face-downs: add the overflow as crystals (cap 10) | **ign** | band_skill_overflow_to_crystals (DISCREPANCY: no flip-effect trigger) |
| `skill:Pastel✽Palettes:与偶像一起#3` | pass CiRCLE: no CiRCLE reward | **ok** | band_skill_no_circle_bonus |
| `skill:Pastel✽Palettes:与偶像一起#4` | turn start: +1 crystal; may then spend 5 to draw 1 | **ok** | band_skill_turn_start_crystal_and_draw |

### Poppin' Party

| clause | meaning | status | tests |
|---|---|---|---|
| `PPP:Popipa#1` | gain 1000 | **ok** | popipa_gains_adds_draws_and_removes |
| `PPP:Popipa#2` | add a Pipopa to the draw pile, draw 1 | **ok** | popipa_gains_adds_draws_and_removes,ix_popipa_pipopa_chain |
| `PPP:Popipa#3` | [移除] this card | **ok** | popipa_gains_adds_draws_and_removes |
| `PPP:[衍生]Pipopa#1` | gain 1000 | **ok** | pipopa_gains_places_and_removes |
| `PPP:[衍生]Pipopa#2` | place a Popipapapipopa on your field | **ok** | pipopa_gains_places_and_removes |
| `PPP:[衍生]Pipopa#3` | [移除] this card | **ok** | pipopa_gains_places_and_removes |
| `PPP:[衍生]Popipapapipopa#1` | [持续](1) passing 东京外/江户川/山吹/星之鼓动/流星堂: +1 crystal (cap 10) | **ok** | popipapapipopa_crystals_on_pass |
| `PPP:[衍生]Popipapapipopa#2` | (2) on a spend/pay: may spend crystals, -150 each (floor 0) | **ok** | popipapapipopa_crystals_reduce_spend |
| `PPP:Returns#1` | [特](1) in the starting deck: deck size +8 | **unc** | — |
| `PPP:Returns#2` | (2) game start: place on your field and borrow another living player's band card | **ok** | returns_placed_at_start_and_borrows_band |
| `PPP:Returns#3` | [持续](1) negates Poppin' Party band (4) | **unc** | — |
| `PPP:Returns#4` | (2) each turn start: may swap the borrowed band card (crystals on the old one go away) | **unc** | — |
| `PPP:Returns#5` | (3) using the borrowed band's active costs 1 star sticker | **unc** | — |
| `PPP:Returns#6` | (4) after your 通用 card's [手] resolves: +1 star sticker | **ok** | returns_grants_sticker_after_general_card |
| `PPP:Tomorrow's Door#1` | the named sequence of 10 tiles | **ok** | door_starts_on_ryuseido_and_walks_the_sequence |
| `PPP:Tomorrow's Door#2` | starts on 流星堂; each time the user passes it, advance one step (last -> user's play area) | **ok** | door_starts_on_ryuseido_and_walks_the_sequence,t07_tomorrows_door_walks |
| `PPP:Tomorrow's Door#3` | while in your play area: another settling on your tile or 梦开始的地方 pays you (houses on 星之鼓动)*100 extra | **ok** | t06_tomorrows_door_surcharge,ix_door_surcharge_on_rent (both now assert paid = R + houses×100 = 900) |
| `PPP:Bang Dream!#1` | +1 crystal on your band card | **ok** | bang_dream_crystal_teleport_build |
| `PPP:Bang Dream!#2` | teleport to any tile you own, may build | **ok** | bang_dream_crystal_teleport_build |
| `PPP:STAR BEAT!#1` | gain 1 layer of 'after the next settle may build on one of your tiles within 5, then drop 1 layer' | **ok** | star_beat_offers_two_moves |
| `PPP:STAR BEAT!#2` | option 1: +2 star stickers; main move = teleport to (45*(players-with-5-in-money + 1)) mod 60, settle | **ok** | star_beat_offers_two_moves,ix_star_beat_counts_the_fives |
| `PPP:STAR BEAT!#3` | option 2: main move = (2*(players-with-5-in-money + 1))d10, settle | **ok** | star_beat_offers_two_moves |
| `PPP:抓到了#1` | move to the tile of the player ahead of you (main move), may build | **ok** | caught_moves_to_player_ahead |
| `PPP:迷宫般的仓库#1` | gate: within 5 of 流星堂 | **ok** | warehouse_moves_from_ryuseido |
| `PPP:迷宫般的仓库#2` | roll 1d10, walk from 流星堂 past that many unowned purchasable tiles (main move; buying is free but demolishes) | **ok** | warehouse_moves_from_ryuseido |
| `PPP:迷宫般的仓库#3` | if you pass 流星堂: force-stop and spend 6000 | **unc** | — |
| `PPP:献给远方的你#1` | teleport to a farthest player's tile and settle | **ok** | far_teleports_to_farthest_player_and_offers_build |
| `PPP:献给远方的你#2` | then may build on one of your buildable tiles among the farthest | **ok** | far_teleports_to_farthest_player_and_offers_build |
| `PPP:仓库里的Random Star#1` | place on own field; shuffle discard+hand into the deck; put a 拍卖撤下来了 on the bottom | **ok** | random_star_sweeps_and_stacks_auction_card |
| `PPP:仓库里的Random Star#2` | [经过] 流星堂: may spend 2 star stickers to force-stop there and settle | **unc** | — |
| `PPP:[衍生]拍卖撤下来了#1` | [特] on entering hand: place on own field; each turn start [移除] the owner's 仓库里的Random Star (then lose 540) | **unc** | — |
| `PPP:[衍生]拍卖撤下来了#2` | [持续] owner's fire cap +1; unaffected by any other effect | **unc** | — |
| `PPP:向着未来的路标#1` | main move = 60 tiles, no settle | **ok** | signpost_moves_60_without_settle_and_loses_1000 |
| `PPP:向着未来的路标#2` | turn end: lose 1000 | **ok** | signpost_moves_60_without_settle_and_loses_1000 |
| `PPP:（香澄）大家我都喜欢哦#1` | [手] place on 星之鼓动山丘 | **ok** | kasumi_card_lands_on_the_hill |
| `PPP:（香澄）大家我都喜欢哦#2` | [持续] another passing it without ending there: force-stop, this goes to the user's discard, +1 crystal on the user's band card | **ign** | m14a/m14b / m15_first_forced_stop_wins (DISCREPANCY: place_raw skips the body / stop not forced) |
| `PPP:（香澄）大家我都喜欢哦#3` | that settle's rent is half | **unc** | — |
| `PPP:（有咲）等等等一下#1` | [手] place on user's field | **ok** | aoki_card_goes_to_field |
| `PPP:（有咲）等等等一下#2` | [持续](1) any event settle not caused by this: +1 crystal | **unc** | — |
| `PPP:（有咲）等等等一下#3` | (2) when an event triggers and this has >=3 crystals: spend 3, defer the event to your turn start, then also draw an event as if from 流星堂 | **unc** | — |
| `PPP:（多惠）寻找更好的声音#1` | teleport to 江户川乐器店 and gain 1 fire | **ok** | megumi_card_teleports_and_gains_fire |
| `PPP:（沙绫）总有一天要给这片天空命名#1` | place on 山吹面包房 | **ok** | saaya_card_lands_on_yamabuki |
| `PPP:（沙绫）总有一天要给这片天空命名#2` | user passing it: at turn end +1 fire; then roll 3d20 and move this to that tile | **ok** | t19_saaya_names_the_sky |
| `PPP:（里美）我的心就像巧克力螺#1` | enter move phase; main move = any tile within absolute distance 1-4, settle | **ok** | rimi_card_offers_distance_1_to_4 |
| `PPP:（里美）我的心就像巧克力螺#2` | [不可阻挡] during that move | **ok** | rimi_card_offers_distance_1_to_4 |
| `skill:户山香澄:非凡之星#1` | another settling on 星之鼓动山丘: +1 fire (init 0, cap 1) | **ok** | kasumi_skill_gains_fire_on_hill_settle |
| `skill:户山香澄:非凡之星#2` | management phase: spend 1 fire, enter move phase, main move = teleport to one of your tiles, may build | **ok** | kasumi_skill_teleports_to_own_tile |
| `skill:市谷有咲:盆栽爱好者#1` | another drawing an event at 流星堂: +1 fire (init 0, cap 2) | **unc** | aoki_skill_spends_fire_for_crystal covers (3) only |
| `skill:市谷有咲:盆栽爱好者#2` | turn end if the main move passed 流星堂 without ending there: draw an event as if from 流星堂 and +1 fire | **unc** | — |
| `skill:市谷有咲:盆栽爱好者#3` | passing 流星堂: spend 2 fire, +1 crystal on your band card | **ok** | aoki_skill_spends_fire_for_crystal |
| `skill:花园多惠:花园警察，出警！#1` | on 领取[CiRCLE奖励]: roll 3d20; if not 星空齿科, place a 多惠兔子 there; passing a rabbit tile removes them all and gains that many fire (init 2, cap 4). Starved by PPP band (2) 「无法获取[CiRCLE奖励]」 in a live PPP game -- the trigger is 领取, which never happens | **ok** | megumi_skill_places_rabbit_on_circle_reward,t28_hanae_rabbits (band skill stripped so 领取 can happen) + megumi_no_rabbit_when_band_2_blocks_the_reward (live outcome) |
| `skill:花园多惠:花园警察，出警！#2` | when another uses a skill [主] or a [手] effect: spend 4 fire to cancel it; you +500, they +2000 | **ign** | c24/c25/l05 (DISCREPANCY: window never opens) |
| `skill:山吹沙绫:焕然一新的天空中#1` | another spends/pays >= 1000 and you hold no saaya mark: they +200, you +1 mark and +1 fire (init 3, cap 5) | **ok** | saaya_skill_offers_on_big_spend |
| `skill:山吹沙绫:焕然一新的天空中#2` | holding a mark: on a move roll lose 1 mark and -1d10 (<=0 -> next purchasable tile) | **unc** | — |
| `skill:山吹沙绫:焕然一新的天空中#3` | on a spend/pay: spend 5 fire, -5000 (floor 0) | **unc** | — |
| `skill:牛込里美:里美的决心#1` | in your turn, after passing any Live House colour tile: at turn end +1 fire (init 1, cap 3) | **unc** | rimi_skill_teleports_to_farthest_agent covers (2) only |
| `skill:牛込里美:里美的决心#2` | management phase: spend 3 fire, teleport to a farthest agent and settle, may build | **ok** | rimi_skill_teleports_to_farthest_agent |
| `skill:Poppin' Party:星之鼓动#1` | pass #1/#16/#31/#46: +1 star sticker; may spend 2 stickers for 1 crystal; may spend 2 crystals for draw 1 or +2000 (once per turn) | **ok** | band_sticker_on_corner_pass,ix_band_sticker_to_crystal_to_cash |
| `skill:Poppin' Party:星之鼓动#2` | no CiRCLE reward | **ok** | band_no_circle_reward |
| `skill:Poppin' Party:星之鼓动#3` | game start: every PP character co-owns 星之鼓动山丘; non-PP settles there pay every unmortgaged PP character; it cannot be force-bought at 2x; only after every PP is bankrupt can it be bought | **ok** | t22_hill_coownership |
| `skill:Poppin' Party:星之鼓动#4` | no building off a main-move settle | **ok** | band_no_build_on_main_move_settle |

### RAISE A SUILEN

| clause | meaning | status | tests |
|---|---|---|---|
| `RAS:R. I. O. T.#1` | [反击] when another's card affects you | **ok** | riot_counter_cycles_hands_and_draws_extra,riot_does_not_negate_the_countered_effect |
| `RAS:R. I. O. T.#2` | everyone discards their hand and redraws the same count; you draw 1 extra | **ok** | riot_counter_cycles_hands_and_draws_extra,c13_riot_and_war_declaration (ignored) |
| `RAS:R. I. O. T.#3` | not offered on board rent | **ok** | riot_not_offered_on_board_rent |
| `RAS:EXIST#1` | place on own field until your next turn start | **ok** | exist_redirects_single_target_card,exist_flips_into_owner_discard |
| `RAS:EXIST#2` | single-target cards (incl. ones aimed at you) retarget to you | **ok** | exist_redirects_single_target_card,exist_does_not_steal_multi_target_card,c11_exist_redirects_single_target |
| `RAS:EXIST#3` | at your next turn start: to your discard; if it did nothing, draw 1 | **ok** | exist_draws_one_when_it_caused_nothing,exist_no_extra_draw_when_it_redirected |
| `RAS:狂乱Hey Kids!!#1` | [反击] when a settle happens on your tile | **ok** | hey_kids_window_on_own_tile_settle,hey_kids_no_window_when_another_settles_your_tile |
| `RAS:狂乱Hey Kids!!#2` | replace the settle: move houses from this tile to your buildable tiles (one side has >=1; each target +1) | **ok** | hey_kids_transfers_one_house,hey_kids_two_houses_to_two_targets |
| `RAS:狂乱Hey Kids!!#3` | house cost is conserved; the excess becomes cash | **ok** | hey_kids_money_when_target_house_is_dearer,hey_kids_money_when_source_house_is_dearer,hey_kids_money_two_houses_to_two_targets |
| `RAS:狂乱Hey Kids!!#4` | then you lose (sum of transferred house costs - count*500) | **ok** | hey_kids_money_when_source_house_is_dearer |
| `RAS:Change the world#1` | place on one of your Live House tiles that has houses | **ok** | change_world_needs_a_livehouse_with_houses |
| `RAS:Change the world#2` | this move roll becomes 3d20; passing a non-yours Live House: +1 crystal | **ok** | change_world_roll_is_3d20_and_collects_crystals |
| `RAS:Change the world#3` | that tile's next charge += 50*(y+1)*n and you gain the same (y = crystals, n = houses) | **ok** | change_world_boosts_the_next_charge |
| `RAS:Change the world#4` | after it fires: to the discard | **ok** | change_world_goes_to_owner_discard |
| `RAS:成为最强#1` | roll 1d10, teleport to the nth Live House by index (>=10 -> the Live House agent) | **ok** | become_strongest_maps_the_roll_to_livehouse_tiles |
| `RAS:成为最强#2` | no Live House owned: teleport to the Live House agent | **ok** | become_strongest_without_livehouses_goes_to_live_house |
| `RAS:PLEASE CHOOSE#1` | [反击] when another settles on a Live House tile | **ok** | please_choose_window_on_livehouse_settle |
| `RAS:PLEASE CHOOSE#2` | they pick: (1) immediately play a card that designates you, or (2) you teleport to their tile (no settle, band skills still see it) | **ok** | please_choose_opt2_teleports_counteractor_without_settle,interaction_please_choose_opt2_is_an_abnormal_move |
| `RAS:练习室里的风暴#1` | place on one of your Live House tiles; user passing CiRCLE: +1 crystal (init 0, cap 3) | **ok** | storm_places_on_own_livehouse_tile,storm_refuses_off_livehouse,storm_gains_crystal_on_circle_pass |
| `RAS:练习室里的风暴#2` | another settling X tiles away (1<=X<=crystals): discard this and force them to settle this tile at (4-X)/4 rent | **ok** | storm_part2_discards_and_forces_a_settle,storm_part2_rent_factor,t08_practice_storm_remote_settle,t08b_practice_storm_x_too_big |
| `RAS:游击演出#1` | [特] at the end of a turn in which this entered the discard and >=1000 was paid to another and you hold no buildable Live House and none is buyable: may place on field and mark your dearest deed as a Live House | **ign** | guerrilla_special_offers_to_mark_a_deed_as_livehouse (DISCREPANCY: window never opens) |
| `RAS:游击演出#2` | [反击] after passing another's Live House and being charged: teleport to an unowned purchasable tile, settle, must buy | **ok** | guerrilla_counter_after_pass_and_charge,guerrilla_teleports_and_must_buy |
| `RAS:游击演出#3` | the chosen tile is the landing tile | **ign** | guerrilla_ends_on_the_chosen_tile (DISCREPANCY: lands one past) |
| `RAS:UNSTOPPABLE#1` | roll 1d6 -> teleport to one of 6 named schools | **ok** | unstoppable_maps_dice_and_pays_out |
| `RAS:UNSTOPPABLE#2` | no settle; counts as the main move | **ok** | unstoppable_does_not_settle |
| `RAS:UNSTOPPABLE#3` | faces 1-3: +2000; 4-6: +1000 | **ok** | unstoppable_maps_dice_and_pays_out |
| `RAS:Repaint#1` | [反击] after another's move roll, in the move phase | **ok** | repaint_reduces_the_move_by_owned_path_tiles,repaint_counts_only_the_original_path |
| `RAS:Repaint#2` | their move distance -X (X = your tiles on their original path) | **ok** | repaint_reduces_the_move_by_owned_path_tiles,m05_repaint_plus_half_speed |
| `RAS:Repaint#3` | their settlement payment is halved | **ok** | repaint_halves_the_settle_payment |
| `RAS:（chuchu）演奏我的音乐吧#1` | place on another player's field with 3 crystals; their turn end loses 1 | **ok** | chuchu_places_on_another_player_with_three_crystals,chuchu_loses_one_crystal_per_turn_end |
| `RAS:（chuchu）演奏我的音乐吧#2` | the next time a holder buys a deed: user +100, this moves to the next player with crystals refilled | **ok** | chuchu_buy_pays_user_and_passes_the_card_on |
| `RAS:（chuchu）演奏我的音乐吧#3` | each such trigger before it enters the discard: +100 to the payout (cap 500) | **unc** | — |
| `RAS:（chuchu）演奏我的音乐吧#4` | at 0 crystals: to the user's discard and the user draws 1 | **unc** | — |
| `RAS:（LOCK）追逐梦想的步伐#1` | before the opening 2 cards: reveal and place on field; game start teleport to 东京外 with 3 [除外] | **unc** | lock_skill / lock_redirects cover (2) only |
| `RAS:（LOCK）追逐梦想的步伐#2` | if the owner's main move passes Bandori station: before settling, the end becomes 旭汤澡堂; then [移除] this | **ok** | lock_redirects_a_move_past_bandori_station,lock_leaves_a_move_that_misses_bandori_alone,m17_lock_redirects_destination |
| `RAS:（MASKING）CRUSH ON THE DRUM!!!#1` | [特] this turn the user's dice-adding cards are unaffected by other effects | **unc** | — |
| `RAS:（MASKING）CRUSH ON THE DRUM!!!#2` | before the move phase: this turn's move roll += Xd20 (X = discard size) | **ok** | crush_adds_xd20_for_the_discard_pile,crush_is_exclusive_to_masking |
| `RAS:（PAREO）渐渐远去的你#1` | gain 2 PAREO marks; your house count counts as higher | **ok** | pareo_card_gives_two_tokens_and_offers_a_house_removal |
| `RAS:（PAREO）渐渐远去的你#2` | may remove 1 house from any tile you own | **ok** | pareo_card_can_remove_a_house |
| `RAS:（和奏瑞依）寄于指尖的执念#1` | when you roll with a fire pot: record the unchosen face | **unc** | no test |
| `RAS:（和奏瑞依）寄于指尖的执念#2` | later spend 1 fire to use a recorded face as that turn's move roll; delete it | **unc** | — |
| `RAS:（和奏瑞依）寄于指尖的执念#3` | multiple faces may be kept | **unc** | — |
| `skill:和奏瑞依:一次又一次竭尽全力#1` | pass CiRCLE: +1 fire (init 1, cap 1) | **ok** | rio_skill_gains_fire_on_circle_pass |
| `skill:和奏瑞依:一次又一次竭尽全力#2` | after any roll: may spend 1 fire to roll again and keep either | **ok** | rio_skill_rerolls_with_a_fire_pot |
| `skill:珠手知由:天才制作人#1` | building is half price; if you built last turn, this turn's build is free | **ok** | chuchu_skill_builds_at_half_price |
| `skill:朝日六花:瞄准目标#1` | your first non-旭汤/non-Live-House deed gains the Live House colour | **ok** | lock_skill_colors_the_first_deed_like_a_live_house |
| `skill:朝日六花:瞄准目标#2` | while you hold an initial Live House, your 旭汤澡堂 gains that colour | **unc** | — |
| `skill:鳰原令王那:梦幻可爱♪女仆#1` | game start: if a PP character is in the game, +1 face-up fan | **unc** | — |
| `skill:鳰原令王那:梦幻可爱♪女仆#2` | when your total houses grow: may spend 1 PAREO mark (init 1, cap 3); every other player [分摊]s you a quarter of (house cost * houses) of your dearest tile | **unc** | TEST-FINDINGS lists this as shallow/uncovered |
| `skill:佐藤益木:与燃烧的红色一起驰骋#1` | passing 白雪学园 or 银河拉面馆: +1 fire (init 0, cap 2) | **ok** | masking_skill_gains_fire_on_named_tiles,masking_skill_gains_fire_when_merely_passing |
| `skill:佐藤益木:与燃烧的红色一起驰骋#2` | before the move phase: spend X fire, this turn's move roll += Xd10 | **ok** | masking_skill_adds_xd10_from_fire_pots |
| `skill:RAISE A SUILEN:UNSTOPPABLE#1` | each turn you may take only the first abnormal-move effect | **ign** | m09_ras_band_first_abnormal_only (DISCREPANCY: no decline window) |
| `skill:RAISE A SUILEN:UNSTOPPABLE#2` | if a non-this-skill effect settles you on a Live House and you are still there at next turn start: your next main move may become a teleport to one of your Live Houses | **ok** | band_skill_offers_teleport_after_a_livehouse_settle |

### Roselia

| clause | meaning | status | tests |
|---|---|---|---|
| `R:（ykn）louder#1` | each play: RiNG price multiplier +5 | **ok** | louder_is_consumed_and_exclusive_to_yukina,louder_is_refused_for_a_non_owner |
| `R:学生会的检查#1` | after the move: if one of your deed tiles is within ±3, before settle may play | **ign** | t12_student_council_check (DISCREPANCY: placement / forced stop) |
| `R:学生会的检查#2` | add a 压 to the draw pile; no building this turn | **ign** | t12_student_council_check |
| `R:学生会的检查#3` | may pay half a house cost to place this on that tile | **ign** | t12_student_council_check |
| `R:学生会的检查#4` | the next non-you passer with a different end force-stops and settles at half rent | **ign** | t12_student_council_check |
| `R:学生会的检查#5` | if it enters the discard with less collected than expected or unsettled: refund the spend | **unc** | — |
| `R:向着顶点#1` | move to the next purchasable Live House tile | **ok** | toward_the_top_moves_to_the_next_purchasable_livehouse |
| `R:向着顶点#2` | if all Live Houses are bought: may pay 1.5x to build on one of yours | **unc** | — |
| `R:曲奇时间#1` | reshuffle the whole discard into the draw pile | **ok** | cookie_time_reshuffles_the_discard_and_pays_500_each |
| `R:曲奇时间#2` | gain 500 * (cards returned) | **ok** | cookie_time_reshuffles_the_discard_and_pays_500_each |
| `R:曲奇时间#3` | empty discard is refused | **ok** | cookie_time_refuses_with_an_empty_discard |
| `R:Fire bird#1` | pay 1600, place on own field with X crystals (your choice) | **ok** | fire_bird_costs_1600_and_gains_chosen_crystals |
| `R:Fire bird#2` | turn end: lose 400 and 1 crystal | **ok** | fire_bird_turn_end_burns_400_and_a_crystal,fire_bird_burns_on_turn_end |
| `R:Fire bird#3` | on field: all your tiles' rent is 1.5x | **ok** | interaction_fire_bird_and_rent |
| `R:Fire bird#4` | crystals empty -> discard | **ok** | fire_bird_turn_end_burns_400_and_a_crystal |
| `R:NFO#1` | roll 1d6; turn end gain 1 [停留] | **ok** | nfo_rolls_1d6_and_stays_at_turn_end |
| `R:NFO#2` | 1: +1500 | **ok** | nfo_six_grants_the_1500_from_result_one |
| `R:NFO#3` | 2: move to one ahead of the nearest player ahead, they split 1000 to you (main move, no settle) | **unc** | — |
| `R:NFO#4` | 3: place on field; your next pay auto-saves 1000 and this discards | **ok** | nfo_three_places_the_card_for_a_later_discount |
| `R:NFO#5` | 4: play a playable card from your discard | **unc** | — |
| `R:NFO#6` | 5: +6 crystals on your character card; each gain may spend 1 for +300 | **unc** | — |
| `R:NFO#7` | 6: all of 1-5 (faces >6 recount from 1) | **ok** | nfo_six_grants_the_1500_from_result_one |
| `R:live前的准备#1` | [反击] when passing 江户川乐器店 | **ok** | live_prep_stops_at_the_edogawa_store_when_passing,interaction_live_prep_vs_a_walk_past_the_store |
| `R:live前的准备#2` | force-stop there and settle | **ok** | live_prep_stops_at_the_edogawa_store_when_passing,m16_live_prep_shortens_path |
| `R:选择自己的舞台#1` | [反击] on suffering an abnormal move effect other than [除外] | **ign** | choose_your_stage_answers_an_abnormal_move (DISCREPANCY: no window on a self-teleport) |
| `R:选择自己的舞台#2` | choose whether this move (or the turn end if you cannot move) settles | **unc** | — |
| `R:（纱夜）弹奏弹奏弹奏，继续弹奏#1` | [反击] at a suitable time: use the player's own skill once | **unc** | no test |
| `R:（亚子）黑暗大魔姬亚子#1` | [反击] when you are about to pay another | **ok** | ako_exclusive_stuns_you_to_dodge_a_payment,interaction_ako_counter_vs_a_rent_payment |
| `R:（亚子）黑暗大魔姬亚子#2` | gain 1 [眩晕]; if played from outside the hand, draw 1 | **shl** | ako_exclusive_stuns_you_to_dodge_a_payment |
| `R:（燐子）Ringing Bloom#1` | place on own field | **ok** | ringing_bloom_places_itself |
| `R:（燐子）Ringing Bloom#2` | all your tiles' house counts count as your maximum, but the inflated ones charge half | **ign** | t10_ringing_bloom (DISCREPANCY: (2)/(3) do not apply — P1 pays the base 260 for 0 houses, want R(3)/2 = 2560) |
| `R:（燐子）Ringing Bloom#3` | after a non-RiNG of yours collects: discard this and gain 500*X (X = houses on the collecting tile) | **unc** | — |
| `R:蓝玫瑰的骄傲#1` | if your Live Houses >= your other tiles: gain 20% of those Live Houses' deed value | **ok** | blue_rose_pride_pays_20pct_when_livehouses_dominate |
| `R:蓝玫瑰的骄傲#2` | else teleport to the next unbought Live House (or RiNG 4, no settle); main move | **ok** | blue_rose_pride_teleports_to_the_next_unbought_livehouse |
| `R:Sprechchor#1` | gate: turn starts on a Live House tile | **ok** | sprechchor_pays_1000_plus_120_per_die_face |
| `R:Sprechchor#2` | roll 1d20, gain 1000 + 120*face | **ok** | sprechchor_pays_1000_plus_120_per_die_face |
| `R:轨迹#1` | [反击][特] when a player goes bankrupt: reveal this | **unc** | no test |
| `R:轨迹#2` | gain any one of their deeds (auto-redeemed) and demolish its houses | **unc** | — |
| `R:必然的联系（莉莎）#1` | [反击] after your skill teleports you | **unc** | no test |
| `R:必然的联系（莉莎）#2` | designate a character on your tile, place this on their play area | **unc** | — |
| `R:必然的联系（莉莎）#3` | your skills on them ignore the 'nearest' limit; then this discards | **unc** | — |
| `R:[衍生] 觉悟#1` | [特] on [移除]: gain 1000 | **ok** | derived_resolve_removes_itself_and_pays_on_removal |
| `R:[衍生] 觉悟#2` | [手] [移除] this card | **ok** | derived_press_pays_1000 |
| `skill:凑友希那:来练习吧#1` | game start: own and start on RiNG 4; first CiRCLE pass has no reward | **ok** | yukina_skill_1_starts_on_ring4_and_owns_it,yukina_skill_1_first_circle_pass_gives_no_reward |
| `skill:凑友希那:来练习吧#2` | if the move passes a RiNG: may stop at the first one | **unc** | — |
| `skill:凑友希那:来练习吧#3` | ending on a RiNG: +1 fire (cap 1); off a RiNG: lose all fire | **ign** | yukina_skill_3_gains_a_fire_pot_on_a_ring (DISCREPANCY: no pot on landing) / yukina_skill_3_loses_fire_pots_off_a_ring |
| `skill:凑友希那:来练习吧#4` | when another passes you: may spend 1 fire to force-stop them and settle | **ign** | interaction_yukina_stop_pot_vs_a_passing_player / m12a/m12b (DISCREPANCY: no prompt) |
| `skill:白金燐子:即使1cm也要前进#1` | pass CiRCLE: +3 fire (init 3, cap 3) | **ok** | rinko_skill_1_gains_three_fire_pots_passing_circle |
| `skill:白金燐子:即使1cm也要前进#2` | usable even while stunned; pre-roll spend X fire to fix the move roll to X*6; that move is immune to abnormal move effects | **ign** | rinko_skill_2_fixes_the_move_roll_to_x_times_six / rinko_skill_2_prompt_is_offered_before_the_move_roll / m10 / m11 (DISCREPANCY: no pre-roll window) |
| `skill:宇田川亚子:对帅气的憧憬#1` | each CiRCLE pass, if you take the draw: look at 2, keep 1, the rest to discard or face-down on your field for 500 | **unc** | no test |
| `skill:宇田川亚子:对帅气的憧憬#2` | taking a CiRCLE reward while a face-down card is on your field: the reward becomes taking that card | **unc** | — |
| `skill:宇田川亚子:对帅气的憧憬#3` | a face-down [反击] on your field may be played at the right time | **unc** | — |
| `skill:冰川纱夜:踏上荆棘之路的觉悟#1` | pass CiRCLE: +2 fire; being passed: +1 fire (init 10, cap 10) | **ok** | sayo_skill_1_gains_fire_pots_passing_circle |
| `skill:冰川纱夜:踏上荆棘之路的觉悟#2` | spend 6 fire to add 1 or 2 to a non-teleport main move | **unc** | — |
| `skill:冰川纱夜:踏上荆棘之路的觉悟#3` | after the first player is eliminated, CiRCLE fire +1 | **unc** | — |
| `skill:今井莉莎:慈爱女神#1` | pass CiRCLE: +1 fire (init 2, cap 2) | **ok** | lisa_skill_1_gains_a_fire_pot_passing_circle |
| `skill:今井莉莎:慈爱女神#2` | before the move phase: spend 1 fire to teleport to the nearest character ahead and settle | **unc** | — |
| `skill:今井莉莎:慈爱女神#3` | before that settle: spend 1 more fire (or give a temporary over-cap fire) to skip the settle entirely | **unc** | — |
| `skill:今井莉莎:慈爱女神#4` | temporary fire expires in 2 turns; using it costs the giver 600 | **unc** | — |
| `skill:Roselia:对音乐的纯粹#1` | your first non-Live-House buy is half price and counts as a Live House until you hold one | **ok** | band_skill_halves_the_first_non_livehouse_purchase,interaction_band_skill_with_a_livehouse_and_a_normal_deed |
| `skill:Roselia:对音乐的纯粹#2` | any Live House buy is half price | **ok** | band_skill_halves_a_livehouse_purchase |

### Sumimi

| clause | meaning | status | tests |
|---|---|---|---|
| `Sumimi:兼顾偶像与乐队#1` | gate: no redeems this turn | **ok** | idol_and_band_gate_redeem_blocks |
| `Sumimi:兼顾偶像与乐队#2` | if money > 3000: no effect | **ok** | idol_and_band_gate_at_3000_or_above |
| `Sumimi:兼顾偶像与乐队#3` | reset money to 3000 | **ok** | idol_and_band_resets_money_to_3000,interaction_idol_and_band_with_black_birthday |
| `Sumimi:Sumimi不会解散哦#1` | gate: no mortgage/redeem this turn and money has no repeated digit | **ok** | sumimi_wont_break_up_requires_different_digits |
| `Sumimi:Sumimi不会解散哦#2` | teleport such that the money after settle has a repeated digit (main move) | **unc** | — |
| `Sumimi:Sumimi不会解散哦#3` | if it still has no repeated digit: return and cancel everything | **unc** | — |
| `Sumimi:一人两个甜甜圈#1` | gain [除外] until another passes your original tile | **ok** | two_donuts_each_gives_exile,interaction_two_donuts_exile_blocks_move |
| `Sumimi:一人两个甜甜圈#2` | then you may teleport between your original tile and their end and settle; you each gain 2 fire (over-cap fire becomes 500 each) | **ok** | m26_two_donuts |
| `Sumimi:现在她是Sumimi的小初啦#1` | [反击] when your money loss this turn is about to exceed your tile's charge | **shl** | now_shes_sumimi_hina_counter (in-hand only) |
| `Sumimi:现在她是Sumimi的小初啦#2` | gain that tile's charge (a RiNG's charge is its base multiplier) | **unc** | — |
| `Sumimi:Sumimi是二人一体的#1` | swap your character card for the other Sumimi and their initial fire | **ok** | sumimi_is_two_in_one_swaps_character |
| `Sumimi:Sumimi是二人一体的#2` | may be used as a [反击] after your move roll | **unc** | — |
| `Sumimi:Here the world#1` | [反击] when someone plays 2 cards in one turn: place on their field | **ok** | here_the_world_places_on_opponent,interaction_generic_encore_vs_sumimi_card |
| `Sumimi:Here the world#2` | their next draw goes face-down on this with 3 crystals; their turn end removes 1; at 0 they take it and you draw 1 | **ign** | c21_here_the_world (DISCREPANCY: does not land on the second card) |
| `Sumimi:Sweet Escape#1` | gate: turn start with the sum of charges within ±2 >= 2000 | **ign** | sweet_escape_gate_requires_high_rent (RULING: 「收费标价」 = tile price (sum 8200 ≥ 2000) or base rent (820 < 2000)?) |
| `Sumimi:Sweet Escape#2` | teleport to a matching-colour non-agent unmortgaged tile of yours-less 周边精选/商店街/东京外, settle, must buy if purchasable (main move) | **unc** | — |
| `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励#1` | before the move roll: this move starts at 小豆岛 | **ok** | childhood_friend_starts_from_shodoshima |
| `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励#2` | after the move: +1 fire | **unc** | — |
| `Sumimi:（真奈）歌唱大赛5连冠#1` | [反击] when you or your tiles are about to be hit by a non-you effect | **shl** | mana_singing_contest_counter (in-hand only) |
| `Sumimi:（真奈）歌唱大赛5连冠#2` | others may play their [反击] as if they were the target; their self-targeting effects aim at you | **unc** | — |
| `Sumimi:（真奈）歌唱大赛5连冠#3` | if a joiner saves you: they draw 1; if nobody joins: you draw 1 | **unc** | c22/c23 cover the general counter, not this card |
| `Sumimi:#L11#1` | place on own field with 2 crystals | **ok** | l11_places_with_two_crystals |
| `Sumimi:#L11#2` | when the band skill would remove a crystal: may remove one here instead; at 0 discard | **unc** | — |
| `Sumimi:#L12#1` | [手] take a Sumimi card from the discard or draw pile into hand; place this on your field | **ok** | l12_hand_places_and_caps_hand |
| `Sumimi:#L12#2` | [持续](1) hand limit -1 | **ok** | l12_hand_places_and_caps_hand |
| `Sumimi:#L12#3` | (2) each fire spent: +1 crystal | **unc** | — |
| `Sumimi:#L12#4` | (3) at 6 crystals: return this to hand | **unc** | — |
| `skill:纯田真奈:甜甜圈爱好者#1` | pass CiRCLE: +1 fire (init 1, cap 2) | **ok** | mana_skill_fire_on_circle_pass |
| `skill:纯田真奈:甜甜圈爱好者#2` | passing a named food tile: before settling may spend 1 fire and pay that tile once; until your next turn end your spends are halved | **unc** | — |
| `skill:三角初华（Sumimi）:成为偶像#1` | pass CiRCLE: +1 fire (init 2, cap 2) | **ok** | hina_sumimi_skill_fire_cap |
| `skill:三角初华（Sumimi）:成为偶像#2` | turn start: may place 1 fire on a 主要街道-colour tile; another passing it and ending pays you 30*X (X = tiles left) and removes the fire | **unc** | — |
| `skill:Sumimi:人气偶像组合#1` | if you gained money from another this turn: at turn end +1 crystal; else clear every crystal | **unc** | sumimi_band_skill_bound is binding-only |
| `skill:Sumimi:人气偶像组合#2` | each crystal raises your charges to others by 100 | **unc** | — |
| `skill:Sumimi:人气偶像组合#3` | you may carry two characters' exclusive cards, but may only play the one matching your character | **unc** | — |

## Interaction surfaces

### Surface list and categories

**`money.pipeline`**

- `flat-add` — gain / lose a fixed amount. Reps: `通用:GREAT#3`, `Mujica:黑色生日#1`, `Sumimi:兼顾偶像与乐队#3`.
- `multiply` — scale an amount by a factor. Reps: `AG:（摩卡）0.5倍速#3`, `R:Fire bird#3`, `skill:Ave Mujica:假面之下的真实#3`.
- `cancel-all` — negate the whole money event. Reps: `CRYCHIC:主唱太拼命了#1`, `MyGO:羽丘的不可思议女孩#4`.
- `cancel-one` — drop one designation of a multi-target pay. Reps: `通用:网络链接异常#2`, `Mor:（小白）#2`.
- `redirect` — send the money to a different player. Reps: `skill:二叶筑紫:交给班长吧#1`, `MyGO:无路矢#3`.
- `clamp-floor` — minimum-0 / minimum-N on a computed amount. Reps: `通用:@Tsugu ycm#5`, `PP:[丸山彩]憧憬的前方#2`, `PP:不要背负期待#4`.
- `replace-loss` — turn a loss into a gain, or defer it. Reps: `Mor:迷茫之蝶们的三全音#2`, `Mor:再次牵起手来#2`, `Sumimi:现在她是Sumimi的小初啦#2`.
- `split-share` — divide an amount across players. Reps: `通用:登上武道馆#1`, `PP:[白鹭千圣]微笑的铁假面#1`, `HHW:（kkr）前往笑容集结的地方！#2`.

**`roll.dice`**

- `dice-set` — change which dice are rolled. Reps: `HHW:运动的天赋#3`, `RAS:（MASKING）CRUSH ON THE DRUM!!!#2`, `skill:佐藤益木:与燃烧的红色一起驰骋#2`.
- `dice-result` — change the numeric result. Reps: `AG:Y.O.L.O#2`, `AG:（绯玛丽）如果并非没问题#2`, `PP:不要背负期待#3`.
- `dice-reroll` — abandon and re-roll. Reps: `Mor:蝴蝶飞舞的星月夜#2`, `CRYCHIC:（立希）即便比不上...#2`, `skill:和奏瑞依:一次又一次竭尽全力#2`.
- `dice-fix` — replace the roll with a fixed formula. Reps: `skill:白金燐子:即使1cm也要前进#2`, `skill:若叶睦（CRYCHIC）:精致的人偶#2`, `Mujica:（初华）我，无畏悲伤#2`.

**`path.move`**

- `pass` — [经过] a named tile. Reps: `skill:月岛麻里奈:礼物还有好多好多哟#1`, `HHW:黑衣人的补给#1`, `skill:Poppin' Party:星之鼓动#1`.
- `forced-stop` — [强制停下] on the path. Reps: `MyGO:（乐奈）有趣的女人#3`, `PPP:（香澄）大家我都喜欢哦#2`, `R:live前的准备#2`.
- `end-rewrite` — change the [移动终点] after the roll. Reps: `RAS:Repaint#2`, `RAS:（LOCK）追逐梦想的步伐#2`, `MyGO:若能再次交汇#2`.
- `start-rewrite` — change the [移动起点] before the move. Reps: `Mor:你的光芒将照亮前路#2`, `skill:宇田川巴:豚骨酱油拉面大姐#2`, `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励#1`.
- `reverse` — move backwards / reverse the move. Reps: `skill:美竹兰:叛逆的红挑染#2`, `skill:仓田真白:向后全速前进#2`, `skill:羽泽鸫:伟大的平凡#2`.
- `teleport` — [传送] to a named or computed tile. Reps: `AG:商店街的青梅竹马#2`, `RAS:UNSTOPPABLE#1`, `MyGO:无路矢#2`.

**`settle`**

- `replace-settle` — swap the landing settle for something else. Reps: `通用:[都筑诗船]Parking Space#2`, `RAS:狂乱Hey Kids!!#2`, `PPP:Tomorrow's Door#3`.
- `skip-settle` — move without settling. Reps: `通用:@Tsugu ycm#3`, `PPP:向着未来的路标#1`, `Mujica:人偶的箱庭#1`.
- `remote-settle` — force a settle on a different tile. Reps: `RAS:练习室里的风暴#2`, `MyGO:（立希）想认真去做#1`.
- `extra-settle` — settle an extra time. Reps: `MyGO:轮符雨#2`, `Mor:高贵的微蓝#2`.

**`counteract.window`**

- `negate-effect` — cancel an effect outright. Reps: `通用:网络链接异常#3`, `通用:安可#2`.
- `negate-one` — cancel one target of a multi-target effect. Reps: `通用:网络链接异常#2`.
- `answer-counter` — counter a counter. Reps: `Mujica:无法将视线移开#1`, `通用:网络链接异常#1`.
- `shut-window` — close the counter window for a period. Reps: `Mujica:骰子已经掷下#1`.
- `join-window` — let a third party also counter. Reps: `Sumimi:（真奈）歌唱大赛5连冠#2`.

**`targeting`**

- `designate` — [指定] a player or tile. Reps: `通用:登上武道馆#3`, `AG:即使夕阳落山#2`.
- `immunity` — cannot be designated / unaffected. Reps: `Mor:夏日合宿#1`, `PP:不要背负期待#1`, `Mor:高贵的微蓝#3`.
- `redirect` — retarget a single-target effect. Reps: `RAS:EXIST#2`.
- `absorb` — take another's status or target onto yourself. Reps: `skill:丰川祥子:请把你们的人生交给我#1`.

**`status`**

- `stay` — [停留]. Reps: `通用:雨啊，快点来吧#3`, `MyGO:轮符雨#1`.
- `stun` — [眩晕]. Reps: `CRYCHIC:初演大成功#2`, `R:（亚子）黑暗大魔姬亚子#2`.
- `exile` — [除外]. Reps: `MyGO:无路矢#1`, `Sumimi:一人两个甜甜圈#1`.
- `unstoppable` — [不可阻挡]. Reps: `skill:三角初华:Imprisoned XII#4`, `PPP:（里美）我的心就像巧克力螺#2`.
- `clear-status` — remove stay/stun/exile. Reps: `MyGO:壱雫空#1`, `skill:高松灯（CRYCHIC）:跌跌撞撞...#2`.

**`crystals.firе`**

- `crystal-add` — put crystals on a card. Reps: `PP:TITLE IDOL#1`, `MyGO:难以复刻的奇迹#1`.
- `crystal-spend` — spend crystals as a cost. Reps: `PPP:[衍生]Popipapapipopa#2`, `MyGO:（灯）不再迷茫#2`.
- `crystal-move` — move crystals between cards. Reps: `Mujica:会被骗着买水晶的人#1`.
- `fire-add` — gain fire pots. Reps: `skill:高松灯:诗超绊#1`, `skill:白金燐子:即使1cm也要前进#1`.
- `fire-spend` — spend fire pots. Reps: `skill:佐藤益木:与燃烧的红色一起驰骋#2`, `skill:要乐奈:投币式停车场的猫#2`.

**`draw`**

- `draw-before` — hook before each draw. Reps: `PP:梦在前方，结彩当下#2`.
- `draw-after` — hook after each draw. Reps: `PP:[若宫伊芙]属于我的武士道！#2`, `skill:千早爱音:重新开始#2`.
- `draw-replace` — replace a draw with something else. Reps: `skill:大和麻弥:朝阳照耀的片刻#2`, `PP:练习生解密指南#3`.
- `draw-curve` — draw until a hand size / discard-and-redraw. Reps: `CRYCHIC:春日影#2`, `RAS:R. I. O. T.#2`.

**`field.tile`**

- `tile-place` — place a card on a tile. Reps: `通用:[都筑诗船]Parking Space#1`, `MyGO:（乐奈）有趣的女人#1`.
- `tile-effect` — change what a tile does. Reps: `HHW:黑衣人的补给#2`, `MyGO:（soyo）混合的颜色#1`.
- `tile-swap` — swap / relabel a tile. Reps: `HHW:笑容大游行#4`, `skill:朝日六花:瞄准目标#1`.
- `field-place` — place a card on a player's [场地]. Reps: `AG:绯红之魂#1`, `RAS:EXIST#1`.

**`turn.boundary`**

- `extra-turn` — grant an extra turn. Reps: `通用:CiRCLE THANKS PARTY!#4`, `Mujica:燃尽前的线香花火#2`.
- `until-turn` — an effect lasting to your next turn start. Reps: `Mor:夏日合宿#1`, `Mor:离心力，不为所动#2`.
- `decay` — a counter that ticks down each turn end. Reps: `AG:（摩卡）0.5倍速#2`, `MyGO:那天的雨#4`.

**`hand`**

- `hand-limit` — change the hand limit. Reps: `PP:不要背负期待#3`, `skill:CRYCHIC:美好的往日幻影#1`.
- `hand-dump` — discard / reshuffle the hand. Reps: `RAS:R. I. O. T.#2`, `MyGO:（灯）不再迷茫#1`.
- `hand-view` — look at / pick from the deck. Reps: `HHW:出发！后台之旅！#1`, `skill:大和麻弥:朝阳照耀的片刻#2`.

**`fans.tokens`**

- `fan-flip` — flip P*P fans. Reps: `PP:同一个梦想#2`, `PP:有你与我在这里共度#4`.
- `fan-spend` — spend fans for an effect. Reps: `skill:丸山彩:With~#2`, `skill:若宫伊芙:天下统一#2`.
- `mark-generic` — other marks (兔子 / croquette / PAREO / CP / 抹茶芭菲). Reps: `skill:花园多惠:花园警察，出警！#1`, `skill:北泽育美:全垒打！#1`, `RAS:（PAREO）渐渐远去的你#1`.

### Surface x category-pair matrix

`covered` names a test; `ignored` names the ignore reason; `missing` has
no test. `negator` rows are the negator x every money category (the
task's explicit request).

| surface | A | B | status | test / reason |
|---|---|---|---|---|
| `money.pipeline` | `flat-add` | `multiply` | covered | l03_rent_through_every_modifier |
| `money.pipeline` | `flat-add` | `cancel-all` | covered | interaction_debut_success_vs_rent (blocks fall) / c20_vocal_too_hard_plus_half_speed |
| `money.pipeline` | `flat-add` | `cancel-one` | covered | c03_counter_to_counter_cancels_designation |
| `money.pipeline` | `flat-add` | `redirect` | covered | no_road_income_goes_to_the_designated_player |
| `money.pipeline` | `flat-add` | `clamp-floor` | covered | m04b_minus_two_clamped_to_zero |
| `money.pipeline` | `flat-add` | `replace-loss` | covered | again_persist_cancels_the_owners_next_payment |
| `money.pipeline` | `flat-add` | `split-share` | covered | interaction_black_birthday_money_stack |
| `money.pipeline` | `multiply` | `cancel-all` | covered | c20_vocal_too_hard_plus_half_speed |
| `money.pipeline` | `multiply` | `cancel-one` | missing | no test halves-or-cancels one leg of a split pay |
| `money.pipeline` | `multiply` | `redirect` | missing |  |
| `money.pipeline` | `multiply` | `clamp-floor` | covered | s09_mujica_band_plus_shiroko |
| `money.pipeline` | `multiply` | `replace-loss` | ignored | c18_tritone_vs_card_payment (DISCREPANCY) |
| `money.pipeline` | `multiply` | `split-share` | covered | interaction_kokoro_circle_bonus_and_the_band_skill |
| `money.pipeline` | `cancel-all` | `cancel-one` | covered | c07_three_deep_encore_negated |
| `money.pipeline` | `cancel-all` | `redirect` | missing |  |
| `money.pipeline` | `cancel-all` | `clamp-floor` | covered | c20_vocal_too_hard_plus_half_speed |
| `money.pipeline` | `cancel-all` | `replace-loss` | covered | interaction_again_cancels_toubudoukan_payment |
| `money.pipeline` | `cancel-all` | `split-share` | covered | interaction_vocalist_counter_vs_big_payment |
| `money.pipeline` | `cancel-one` | `redirect` | missing |  |
| `money.pipeline` | `cancel-one` | `clamp-floor` | ignored | net_cancels_one_target_not_all (TODO(ABI)) |
| `money.pipeline` | `cancel-one` | `replace-loss` | missing |  |
| `money.pipeline` | `cancel-one` | `split-share` | ignored | c05_cancels_one_designation_of_four |
| `money.pipeline` | `redirect` | `clamp-floor` | missing |  |
| `money.pipeline` | `redirect` | `replace-loss` | missing |  |
| `money.pipeline` | `redirect` | `split-share` | covered | tsukushi_skill1_redirects_a_gain |
| `money.pipeline` | `clamp-floor` | `replace-loss` | covered | expect_pay_plus_100 |
| `money.pipeline` | `clamp-floor` | `split-share` | missing |  |
| `money.pipeline` | `replace-loss` | `split-share` | missing |  |
| `roll.dice` | `dice-set` | `dice-result` | covered | m03_eve_then_yolo (ignored) / m04_minus_two_plus_yolo |
| `roll.dice` | `dice-set` | `dice-reroll` | missing |  |
| `roll.dice` | `dice-set` | `dice-fix` | ignored | m11_rinko_plus_yolo / m10_rinko_while_stunned |
| `roll.dice` | `dice-result` | `dice-reroll` | covered | starry_owner_may_reroll_the_move_roll |
| `roll.dice` | `dice-result` | `dice-fix` | ignored | m11_rinko_plus_yolo |
| `roll.dice` | `dice-reroll` | `dice-fix` | missing |  |
| `path.move` | `pass` | `forced-stop` | covered | m15_first_forced_stop_wins (ignored) / m16_live_prep_shortens_path |
| `path.move` | `pass` | `end-rewrite` | covered | m17_lock_redirects_destination |
| `path.move` | `pass` | `start-rewrite` | covered | m21_tomoe_shifts_start |
| `path.move` | `pass` | `reverse` | covered | m20a_mashiro_backwards_past_circle |
| `path.move` | `pass` | `teleport` | covered | m29_circle_reward_winners |
| `path.move` | `forced-stop` | `end-rewrite` | missing |  |
| `path.move` | `forced-stop` | `start-rewrite` | missing |  |
| `path.move` | `forced-stop` | `reverse` | missing |  |
| `path.move` | `forced-stop` | `teleport` | ignored | t24_parking_space / t13_smile_parade |
| `path.move` | `end-rewrite` | `start-rewrite` | covered | m17_lock_redirects_destination |
| `path.move` | `end-rewrite` | `reverse` | missing |  |
| `path.move` | `end-rewrite` | `teleport` | covered | m06_reunion_20_tile_cap (ignored) |
| `path.move` | `start-rewrite` | `reverse` | covered | m20b_ran_backwards_no_reward |
| `path.move` | `start-rewrite` | `teleport` | covered | light_move_starts_from_tsukinomori |
| `path.move` | `reverse` | `teleport` | missing |  |
| `settle` | `replace-settle` | `skip-settle` | covered | t06_tomorrows_door_surcharge |
| `settle` | `replace-settle` | `remote-settle` | covered | t08_practice_storm_remote_settle (hey_kids family still ign) |
| `settle` | `replace-settle` | `extra-settle` | missing |  |
| `settle` | `skip-settle` | `remote-settle` | missing |  |
| `settle` | `skip-settle` | `extra-settle` | covered | m23_human_boost_hit_by_stop |
| `settle` | `remote-settle` | `extra-settle` | missing |  |
| `counteract.window` | `negate-effect` | `negate-one` | covered | c07_three_deep_encore_negated |
| `counteract.window` | `negate-effect` | `answer-counter` | covered | c03_counter_to_counter_cancels_designation |
| `counteract.window` | `negate-effect` | `shut-window` | ignored | c14_dice_cast_shuts_windows |
| `counteract.window` | `negate-effect` | `join-window` | covered | c22_manas_counter_nobody_joins / c23 |
| `counteract.window` | `negate-one` | `answer-counter` | covered | c05_cancels_one_designation_of_four (ignored) |
| `counteract.window` | `negate-one` | `shut-window` | missing |  |
| `counteract.window` | `negate-one` | `join-window` | missing |  |
| `counteract.window` | `answer-counter` | `shut-window` | covered | c15_dice_cast_itself_countered |
| `counteract.window` | `answer-counter` | `join-window` | covered | c13_riot_and_war_declaration (ignored) |
| `counteract.window` | `shut-window` | `join-window` | missing |  |
| `targeting` | `designate` | `immunity` | covered | c10_summer_camp_vs_targeting / s01_exiled_cannot_be_designated |
| `targeting` | `designate` | `redirect` | covered | c11_exist_redirects_single_target |
| `targeting` | `designate` | `absorb` | ignored | s08_sakiko_absorbs_statuses |
| `targeting` | `immunity` | `redirect` | ignored | c12_exist_vs_summer_camp |
| `targeting` | `immunity` | `absorb` | missing |  |
| `targeting` | `redirect` | `absorb` | missing |  |
| `status` | `stay` | `stun` | covered | s06_izana_clears_statuses |
| `status` | `stay` | `exile` | covered | s01_exiled_cannot_be_designated |
| `status` | `stay` | `unstoppable` | covered | m30_unstoppable_walks_through_stay |
| `status` | `stay` | `clear-status` | covered | s06_izana_clears_statuses |
| `status` | `stun` | `exile` | covered | s02_stunned_cannot_pay / s01 |
| `status` | `stun` | `unstoppable` | ignored | m10_rinko_while_stunned |
| `status` | `stun` | `clear-status` | covered | s06_izana_clears_statuses |
| `status` | `exile` | `unstoppable` | missing |  |
| `status` | `exile` | `clear-status` | covered | s06_izana_clears_statuses |
| `status` | `unstoppable` | `clear-status` | missing |  |
| `crystals.firе` | `crystal-add` | `crystal-spend` | covered | ix_band_sticker_to_crystal_to_cash |
| `crystals.firе` | `crystal-add` | `crystal-move` | covered | crystal_move_opens_source_and_target_prompts |
| `crystals.firе` | `crystal-add` | `fire-add` | ignored | l07_extra_turn_cascade (DISCREPANCY: the last 线香花火 removal nets 1 [眩晕], want 2) |
| `crystals.firе` | `crystal-add` | `fire-spend` | missing |  |
| `crystals.firе` | `crystal-spend` | `crystal-move` | ignored | c26_immunity_vs_crystal_mover |
| `crystals.firе` | `crystal-spend` | `fire-add` | missing |  |
| `crystals.firе` | `crystal-spend` | `fire-spend` | covered | MyGO:（灯）不再迷茫#2 (ignored: crystal-as-fire) |
| `crystals.firе` | `crystal-move` | `fire-add` | covered | interaction_crystal_move_with_band_and_skills |
| `crystals.firе` | `crystal-move` | `fire-spend` | missing |  |
| `crystals.firе` | `fire-add` | `fire-spend` | covered | tsugu_skill_spends_fire_and_pairs |
| `draw` | `draw-before` | `draw-after` | ignored | l06_one_draw_several_effects (RULING: capture vs auto-play) |
| `draw` | `draw-before` | `draw-replace` | missing |  |
| `draw` | `draw-before` | `draw-curve` | covered | dream_ahead_crystal_per_draw_cap_5 |
| `draw` | `draw-after` | `draw-replace` | covered | maya_skill_2_watch_and_pick |
| `draw` | `draw-after` | `draw-curve` | ignored | l06_one_draw_several_effects (RULING: capture vs auto-play) |
| `draw` | `draw-replace` | `draw-curve` | missing |  |
| `field.tile` | `tile-place` | `tile-effect` | covered | t14_black_clothes_supply |
| `field.tile` | `tile-place` | `tile-swap` | ignored | t13_smile_parade |
| `field.tile` | `tile-place` | `field-place` | covered | t20_clear_cp |
| `field.tile` | `tile-effect` | `tile-swap` | ignored | t16_soyo_mixed_colors |
| `field.tile` | `tile-effect` | `field-place` | covered | t06_tomorrows_door_surcharge |
| `field.tile` | `tile-swap` | `field-place` | missing |  |
| `turn.boundary` | `extra-turn` | `until-turn` | ignored | l07_extra_turn_cascade (DISCREPANCY: the last 线香花火 removal nets 1 [眩晕], want 2) |
| `turn.boundary` | `extra-turn` | `decay` | ignored | l07_extra_turn_cascade (DISCREPANCY: the last 线香花火 removal nets 1 [眩晕], want 2) |
| `turn.boundary` | `until-turn` | `decay` | covered | m09_ras_band_first_abnormal_only (ignored) |
| `hand` | `hand-limit` | `hand-dump` | covered | interaction_band_hand_limit_vs_draw |
| `hand` | `hand-limit` | `hand-view` | covered | guide_hand_limit_and_turn_end_crystal |
| `hand` | `hand-dump` | `hand-view` | covered | believe_in_you_costs_800_and_swaps_hand_cards |
| `fans.tokens` | `fan-flip` | `fan-spend` | covered | ix_aya2_discount_vs_rent |
| `fans.tokens` | `fan-flip` | `mark-generic` | missing |  |
| `fans.tokens` | `fan-spend` | `mark-generic` | missing |  |
| `money.pipeline` | `negator` | `flat-add` | covered | c20_vocal_too_hard_plus_half_speed (主唱太拼命了 is the negator) |
| `money.pipeline` | `negator` | `multiply` | covered | c20_vocal_too_hard_plus_half_speed |
| `money.pipeline` | `negator` | `cancel-one` | covered | c03_counter_to_counter_cancels_designation |
| `money.pipeline` | `negator` | `redirect` | missing |  |
| `money.pipeline` | `negator` | `clamp-floor` | covered | expect_pay_plus_100 |
| `money.pipeline` | `negator` | `replace-loss` | covered | c17_holding_hands_again_cancels_payment |
| `money.pipeline` | `negator` | `split-share` | covered | interaction_vocalist_counter_vs_big_payment |
| `targeting` | `immunity` | `designate` | covered | s01_exiled_cannot_be_designated / c10 |

**Pair coverage (as of 2026-10-06 HEAD): 71/120 = 59% covered,**
14 ignored (12%), 35 missing (29%).
Counting `ignored` as partly covered: 71%.

### Order-dependent pairs and new RULING questions

The consolidated open-rulings list is
[TEST-FINDINGS.md](TEST-FINDINGS.md) §6 ("Ambiguities needing a ruling") --
it carries the R-1..R-7 ids used here and in
[CROSS-TESTS.md](CROSS-TESTS.md) §G12. The bullets below are the *design*
record (what the pair is and a suggested order); the ruling list is not
duplicated here.

- **money.pipeline multiply x multiply** — Fire bird 1.5x vs HHW band double-pay vs 摩卡 half-pay vs Ave Mujica 1.5x. RULING: multiply order is unstated. t11_fire_bird_plus_hhw_double is already `RULING: order of Fire bird 1.5x and HHW band (1) doubling`. Suggest: flat-add, then each multiply in play order (field cards oldest first, then band, then character), then clamp-floor. Record as a Q.
- **money.pipeline flat-add x multiply** — FEVER! +X vs a 1.5x rent multiplier (Fire bird). RULING: does FEVER!'s +X land before or after the rent multiplier? l03_rent_through_every_modifier picks one order; the sheet does not.
- **money.pipeline cancel-one x split-share** — 网络链接异常 drops one designation of 登上武道馆 (a split pay). RULING: after dropping one target, does X recompute from the remaining alive-others? Sheet says X is set before the [指定]; assert the old X.
- **roll.dice dice-result x dice-result** — Y.O.L.O +1d4 vs 不要背负期待 -2 vs Eve's +Yd4. RULING: stack order of additive dice modifiers. inter_yolo_pushes_haneoka_over_20 (CROSS-AGENT) is waiting on this.
- **path.move pass x end-rewrite** — a pass effect that rewrites the end (LOCK) while a later pass effect would also fire. rule 86 covers it, but only if the rewrite is processed per-tile. m17 covers LOCK alone; the pair with a second rewriter is missing.
- **counteract.window answer-counter x join-window** — who may still join a round after a counter-to-a-counter has been declared. RULING: the sheet's 「所有玩家同意所有对一个时点的[反击]已发动后可对新的时点」 does not say whether a late joiner may enter the closed round. c13 is the closest.
- **turn.boundary extra-turn x until-turn** — an effect 'until your next turn start' while extra turns are granted. RULING: does the window close on the first turn start after, or after the whole extra-turn cascade? l07 picks one.

## Top 25 uncovered high-risk clauses / pairs

Ranked by how much a wrong answer would swing a real game, with money,
movement and counters first.

| # | clause / pair | area | why it is risky |
|---|---|---|---|
| 1 | `AG:即使夕阳落山#5` | money / multi-target pay | the X formula and the house demolition are only shallowly tested; the payment itself is unasserted. A wrong divisor silently changes every 4-player game. |
| 2 | `通用:网络链接异常#2` | money / cancel-one | still cancels every designation. Multi-target pays are the most common money event; this is both a DISCREPANCY and an ABI hole (TODO(ABI)). |
| 3 | `Mujica:祥，移动#3` | money / multiply | halved settlement pay is unimplemented (720 not 360). A forced move into a rent tile is a common mid-game swing. |
| 4 | `Mor:秘密与青春的虹彩#2` | money / multiply | the 1.5x case settles at 0. Grade data is missing, so the pair is also an untestable-grade hole. |
| 5 | `Sumimi:现在她是Sumimi的小初啦#2` | money / replace-loss | the whole point of the card (gain the tile charge) is untested; only the in-hand presence is. |
| 6 | `HHW:运动的天赋#2` | money / flat-add + decay | the per-roll money ladder (700 down to 100) is untested; a wrong floor breaks every HHW game. |
| 7 | `RAS:（chuchu）演奏我的音乐吧#3` | money / flat-add | the +100-per-trigger up-to-500 growth is untested. |
| 8 | `skill:山吹沙绫:焕然一新的天空中#3` | money / cancel-one | the -5000 spend reducer is one of the biggest single money levers in the game and is untested. |
| 9 | `PP:[丸山彩]憧憬的前方#2` | money / clamp-floor | the exclusive's whole [持续] is unimplemented; least-money detection is a cross-player query nobody tests. |
| 10 | `MyGO:壱雫空#2` | money / split-share | charges the wrong set of players. Everyone-pays-per-type is a 4+ player swing. |
| 11 | `path.move end-rewrite x forced-stop` | path | no test puts a forced stop and an end-rewrite on the same walk; rule 86's recompute is only tested one-sided. |
| 12 | `MyGO:若能再次交汇#2` | path / end-rewrite | the re-roll loop crashes (`world_mut` while a routine is pending). This is a hard engine bug on a common [反击]. |
| 13 | `AG:回家的路上绕个道#2` | path / start-rewrite | depends on an Afterglow (2) that does not exist as a standalone rule; the card's main effect is unreachable. |
| 14 | `skill:宇田川巴:豚骨酱油拉面大姐#3` | path / start-rewrite | the overshoot-refill and the on-tile ban are untested; the shift itself is only tested in isolation. |
| 15 | `Mujica:无法将视线移开#1` | counteract / answer-counter | does not force the counter-user to move, and cannot be played as a counter (`err.play_phase`). This breaks the whole counter-war ladder (l01, l04). |
| 16 | `Mujica:骰子已经掷下#4` | counteract / shut-window | placed twice and does not actually shut the windows. Every 'no counters this turn' clause is therefore untrustworthy. |
| 17 | `skill:花园多惠:花园警察，出警！#2` | counteract / negator | the cancel window never opens (c24, c25, l05). One of the few skill-level negators. |
| 18 | `R:选择自己的舞台#1` | counteract / abnormal-move | no window on a self-inflicted [传送]; the 'choose whether to settle' half is also untested. |
| 19 | `AG:（鸫）微小的『能做到』的事#2` | roll / dice-fix | 'settle at theoretical max/min' is an unusual dice category and has no test beyond a harness replay. |
| 20 | `skill:白金燐子:即使1cm也要前进#2` | roll / dice-fix | no pre-roll pot window at all (m10, m11). Fixed-X*6 is a whole move-rewrite. |
| 21 | `turn.boundary extra-turn x until-turn` | turn boundaries | l07 picks an order the sheet does not settle; a wrong pick changes every 'until your next turn' shield. |
| 22 | `PP:梦在前方，结彩当下#5` | counteract / buy | the [共鸣][反击] buy-at-turn-end is untested and needs a buy window the harness barely has. |
| 23 | `R:轨迹#2` | settlement / bankruptcy | the bankruptcy payout (take a deed, demolish) is completely untested; bankruptcy is rare but decisive. |
| 24 | `HHW:（薰）怪盗hello happy#4` | path / dice-set | rewrites everyone's main move into 1d2+1d10 while on a tile; zero coverage and a big path-category hole. |
| 25 | `skill:松原花音:真正的迷子#2` | roll / dice-set | move = |a-b| of two rolls is a dice-set category with no test; it also interacts badly with every dice-result modifier. |

## Harness gaps

These block whole classes of clause from ever being asserted.

* ~~Game-start hooks do not reach field (skill) cards.~~ **Landed:**
  `game_start_hooks` dispatches `DeckBeforeGame` / `DeckAtGameStart` to every
  effect source (field cards incl. skills, then piles / hand). 「初始N」 fire
  pots, the P*P fan init and the start-tile rewrites are unblocked.
* ~~Pay / loss counteract windows open only for rent.~~ **Landed:** the money
  pipeline raises the effect window on every money movement (print / delete /
  pay-player), pre-split and post-split per payer/payee pair.
* ~~No per-draw hook: `ctx::draw` does not raise `drew`.~~ **Landed:** a
  card-driven draw logs to `draw_log` and the commit path raises `drawn` /
  `drew` once per single card. `rb_gap_res::g28_*` is re-tagged: Maya (2)'s
  replacement draw does not route the kept card through `drew`.
* ~~「无法获取[CiRCLE奖励]」 suppression.~~ **Landed:** the [经过] reward is
  `tile:circle`'s Pass entry (`ctx::settle_circle_reward`), and suppression is
  `prop::NO_REWARD` on the rule instance -- the tile's own instance (a
  walk-scoped veto) and the passing player's field instance (a per-player
  veto). `plan::set_no_circle_reward` and `state_key::NO_CIRCLE_REWARD` are
  gone. See [TILES.md](../TILES.md).
* No static targeting query, so 「取消其对目标之一的[指定]」 cannot drop one designation (`TODO(ABI)` in `rb_money.rs`).
* No gain-event observer, so 「本回合内你每获得过一次资金」 (育美 (2)) is untestable.
* No grade data in `data/`, so 学妹 / 同级生 / 学姐 (秘密与青春的虹彩) is untestable.
* `use_skill` runs only a card's first `On::Play` entry, so a second activation (MyGO band draw-half) is unreachable.
* `place_raw` skips the play body, so several arrange-only tests never exercise the effect they name.
* No token/marks setter in the harness; tests poke `world_mut()` directly, which makes mark-count clauses brittle.
* Loaded dice are a global queue that card rolls also consume; a card that rolls in the middle of a move silently eats the next face.
* No bankruptcy / auction driver, so 破产-payout clauses (轨迹, ONE OF US (9), PPP band (3) endgame) are untestable.

## Hand-off notes

### Q3 — fuzzer + metamorphic checks

Bias the generator at these surfaces / pairs, in this order:

* `money.pipeline`: every generator seed should build a rent event and stack at least two of {flat-add, multiply, clamp-floor, replace-loss} on it. The multiply x multiply and multiply x clamp-floor pairs are the least settled (see the RULING list).
* `money.pipeline` `cancel-one` against `split-share`: the multi-target pay is the single most common money event and the ABI cannot express the drop-one case yet. Fuzzing will keep rediscovering it; treat it as a known hole until `TODO(ABI)` lands.
* `roll.dice`: stack {dice-set, dice-result, dice-fix} on one move roll. Y.O.L.O x Eve x 不要背负期待 x 白金燐子 is the 4-way. The order question is open (inter_yolo_pushes_haneoka_over_20 is waiting on a ruling).
* `path.move`: a walk that hits {pass, forced-stop, end-rewrite} in one path. Rule 86 says recompute the path; the pair forced-stop x end-rewrite is now `rb_gap_move::g11_forced_stop_beats_lock_end_rewrite` (`RULING`, see [TEST-FINDINGS.md](TEST-FINDINGS.md) §6).
* `counteract.window`: nest 3+ deep with a `shut-window` in the middle (骰子已经掷下) and a `join-window` (真奈). The close-the-round semantics are under-specified.
* `status`: stay gained mid-move, plus `unstoppable` and `clear-status`. `rain_stay_blocks_this_turn_move` is currently a DISCREPANCY and also an ambiguity (ileuxali checks only at branch selection).
* `turn.boundary` `extra-turn` x `until-turn`: a shield that ends 'at your next turn start' while extra turns are being granted. The order is a RULING.
* `fans.tokens` `fan-flip` x `mark-generic`: almost no test mixes P*P fans with 兔子 / croquette / PAREO marks. Cheap to generate, high chance of a real bug.

### Q4 — unintended interactions (footprints)

`docs/rulebook/footprints.json` is `{rule_id: {writes, reads, triggers}}`,
derived from the sheet text plus the manifest hook kinds. The intended use:
a rule may only touch the surfaces listed in `writes`, only observe those
in `reads`, and only fire on those `triggers`. Any engine trace that shows
a write outside `writes` is an unintended interaction.

Notes on the encoding:

* `money`, `roll.result`, `roll.set`, `path.position`, `path.start`,
  `path.direction`, `path.stop`, `tile.*`, `crystals`, `fire`, `hand`,
  `draw.pile`, `discard.pile`, `field`, `marks`, `fans`, `status.*`,
  `cp`, `sticker`, `counter.window`, `effects`, `state` are the surface keys.
* `triggers` uses the manifest's `OnKind:TriggerKind` pairs from
  `target/scratch/rb/manifest.txt`.
* A rule whose `writes` includes `effects` is a negator / shield (安可,
  网络链接异常, 离心力, EXIST). Those are the ones to watch hardest: they
  are allowed to touch another rule's write, and a footprint violation
  there is exactly an unintended interaction.

### Part C mechanical checks

* **Rule ids with no test file mentioning them at all (9):**
  `AG:无论是何种颜色的夕阳`, `AG:（巴）商店街的救世主`,
  `HHW:（薰）怪盗hello happy`, `HHW:（花音）Wacha Mocha 啪嗒进行曲`,
  `HHW:（美咲）`, `skill:濑田薰:梦幻的王子殿下`,
  `RAS:（和奏瑞依）寄于指尖的执念`, `R:（纱夜）弹奏弹奏弹奏，继续弹奏`,
  `R:必然的联系（莉莎）`. The exclusive cards `PP:[大和麻弥]可能性为∞`
  and `PP:[冰川日菜]会发出怎样的声音呢？` are also never played (their
  *skills* are tested).

* **CROSS-TESTS.md §1-§5 landing:** 104 designed cases
  (C1-C27, M1-M30, T1-T29, S1-S10, L1-L8) expanded into **111 tests**
  (the a/b variants: m04b, m06b, m12a/b, m14a/b, m20a/b, m28a/b, t08b).
  **70 passing, 41 `#[ignore]`** (39 DISCREPANCY + 2 RULING),
  0 designed-but-unimplementable (grep of `#[test]` / `#[ignore` attributes,
  2026-10-06 working tree). The 2 RULING ones are
  `t04_anon_link_mortgaged_partner` and `t11_fire_bird_plus_hhw_double`.
  Per-file tests/ignored: `rb_cross_chain.rs` 27/11, `rb_cross_move.rs`
  36/13, `rb_cross_tiles.rs` 30/11, `rb_cross_status.rs` 10/2,
  `rb_cross_long.rs` 8/4.


## Fuzz coverage

Interaction fuzzer + metamorphic checks (`crates/game-rules/tests/fuzz_interactions.rs`,
modules under `crates/game-rules/tests/fuzz/`). Black-box: `rules/cards/`,
`rules/skills/`, `rules/fixtures/` and `target/scratch/tainted/` were never
opened; the generator draws card ids from the shipped manifest at runtime
(`ruleset().cards()`, `TEST:*` fixtures excluded) and biases pairs through
`docs/rulebook/footprints.json` shared-`writes` surfaces.

### Iterations and runtime

| run | iters x turns | acts | prompts | counteracts | co-occurring pairs | wall clock |
|---|---|---|---|---|---|---|
| default (`FUZZ_ITERS` unset) | 900 x 4 | 18,621 | 1,719 | 356 | 1,243 | 52 s (whole file ~60 s) |
| soak `FUZZ_ITERS=2000` | 2000 x 4 | ~41k | ~3.8k | ~790 | ~2.6k | ~85 s |

Long soak: `FUZZ_ITERS=5000 cargo test -p game-rules --test fuzz_interactions -- --nocapture`.
Default is CI-cheap and lands in the 1-2 minute band.

### Card pairs co-occurring in a resolved chain or turn

1,243 distinct unordered pairs at the default. A pair counts when both card
ids are named by the same turn's event log (played, declared in a counteract
window, or named by a trigger). Sample:

* `AG:Y.O.L.O` x `skill:凑友希那:来练习吧`
* `AG:回家的路上绕个道` x `HHW:爱心义演`
* `AG:回家的路上绕个道` x `MyGO:若能再次交汇`
* `AG:回家的路上绕个道` x `RAS:狂乱Hey Kids!!`
* `AG:宣战布告` x `PPP:[衍生]Popipapapipopa`
* `AG:朝同一片天空迈进` x `Mujica:祥，移动`

Regenerate the full set with `FUZZ_ITERS=... cargo test -p game-rules --test
fuzz_interactions fuzz_interactions_invariants -- --nocapture` (the
`sample co-occurring pairs` line names the first 12; the count is in the
`fuzz:` line). The generator biases 75% of partner draws toward cards sharing
a `footprints.json` `writes` surface with the first card of the iteration.

### Surfaces exercised

The generator's footprint bias reaches every `writes` key in
`footprints.json` that a random 900-iter draw touches, and the driver plays
the full action set (play a hand card, use a skill, roll, buy, build,
mortgage/redeem, end) with every prompt kind answered legally (choice, tile,
pick, mortgage, auction, mulligan, counteract declarations). Concretely
covered by construction:

| surface | how |
|---|---|
| `money.pipeline` | every rent / buy / build / mortgage / redeem / card pay goes through the one `money()` pipeline; the ledger invariant checks it after every turn |
| `roll.dice` | `roll` + loaded dice from the seed + cards that rewrite the roll |
| `path.move` | rolls, teleports, reverse, forced stops land in the same turns as pass effects |
| `settle` | land / rent / agent / CiRCLE reward all run in the driver |
| `counteract.window` | 356 counteract declarations at the default, including multi-round nests |
| `targeting` | multi-target pays (`通用:登上武道馆`, `AG:即使夕阳落山`) + immunity (`Mor:夏日合宿`) in the metamorphic check |
| `status` | stay / stun / exile arranged randomly and ticked by turn flow |
| `crystals.fire` | fire pots arranged with caps; crystals ride field cards (FEVER!, `AG:绯红之魂`) |
| `draw` | draw-heavy cards (`CRYCHIC:春日影`, `通用:10次招募`, `HHW:出发！后台之旅！`) reached by random play |
| `field.tile` | field placement (`AG:（摩卡）0.5倍速`, FEVER!, `通用:[都筑诗船]Parking Space`) + deeds/houses/mortgages |
| `turn.boundary` | extra turns (`通用:CiRCLE THANKS PARTY!`) and turn-end decay run in the driver |
| `hand` | hand-limit and dump cards reached by random play |
| `fans.tokens` | P*P fans and generic marks ride the arrangement |

### Per-kind trap and violation counts (default run)

| kind | count | filed as |
|---|---|---|
| fire pot exceeded its declared cap | 39 | `rb_fuzz_found::fire_over_cap` |
| money ledger does not close | 12 | `rb_fuzz_found::money_ledger_gap` |
| `[停留]` went negative | 1 | `rb_fuzz_found::negative_status_leak` |
| settle loop did not come to rest | 2 | `rb_fuzz_found::settle_loop_stuck` |
| `log.card_trap` | 0 | (`rb_fuzz_found::card_trap_reachable`; `TEST:recurse` is excluded from the pool) |
| determinism / save-restore | 0 | (`rb_fuzz_found::determinism_break`, `rb_fuzz_found::save_restore_break`) |

### Findings

All violations are filed as `#[ignore = "DISCREPANCY: ..."]` tests in
`crates/game-rules/tests/rb_fuzz_found.rs` with a minimal repro. The fuzzer's
skip-list (`fuzz::KNOWN_FINDINGS`) recognises their signatures so a default
run stays green.

1. ~~**`money_ledger_gap`**~~ **Fixed (2026-10-06).** `gain_fixed` (三全音's
   「立刻获得此次失去的资金金额」) now logs a `gain` bank leg. Other money
   ledger gaps may remain on other seeds (the `KNOWN_FINDINGS` entry is
   retained); the filed repro passes. Documented structural exclusions (not
   findings): `bankrupt` (cashes mortgages without logging them) and auction
   bid escrow (direct `money -= bid`).

2. ~~**`negative_status_leak`**~~ **Fixed (2026-10-06).** `state_set` /
   `state_add` now floor status counters at 0.

3. ~~**`fire_over_cap`**~~ **Fixed (2026-10-06).** `state_set` / `state_add`
   enforce `StateVar.max > 0` as a real cap; `fire_pot` no longer wipes cap
   raises (`auction_pulled`'s +1 survives).

4. ~~**`settle_loop_stuck`**~~ **Fixed (2026-10-06).** The state-clamping fix
   removed the cascade. The `KNOWN_FINDINGS` entry is retained for other
   seeds; the filed repro passes.

5. ~~**`money_depth_departs_from_sheet`**~~ **Fixed (2026-10-06).**
   Nested money opens its [反击] windows at every depth (the sheet has no
   depth limit). The termination argument is `Cx::reentrant_hooks` (a hook
   cannot re-trigger on its own money movement). `MAX_MONEY_DEPTH = 32` is
   a safety cap that **panics** loudly. See
   [ENGINE.md](../ENGINE.md) "Money pipeline depth".

   Note: `fuzz_interactions_invariants` (the main driver) was red at the
   2026-10-06 count -- concurrent work; the eight metamorphic / determinism
   / probe tests in the same file were green.

6. ~~**`returns_prompt_storm`**~~ **Fixed (2026-10-08).** Fuzz soak iter 1517
   (`seed=10174556463119430459`) hit `drain_prompts: prompts never stopped`
   around PPP:Returns' 「[拥有者]每回合开始时选择一个其他存活玩家的团卡」.
   Root cause was **not** the card: the fuzz arrange wrote `[晕眩]` through
   the raw `state_set` (no `StateVar::expires`), `tick_state` only wears an
   item down when it names its tick, so both seats stayed stunned forever --
   every turn skipped, and the turn-start ask raised one prompt per skipped
   turn. `MatchPlayer::state_set` now stamps the rulebook's tick
   (「玩家的每回合结束时移除一层」; `stunStart` → turn start) on any stay /
   stun / stunStart write that has none. No `KNOWN_FINDINGS` skip entry:
   the fix is general and a future prompt storm is a new finding. Pinned by
   `rb_fuzz_found::returns_prompt_storm`, `rb_ppp::
   returns_turn_start_ask_survives_stun_skip`, and
   `state::tests::status_writes_carry_the_books_tick`. See
   [GUARDS.md](../GUARDS.md) §11.6.

### Metamorphic checks (all green at the default)

| check | what it asserts |
|---|---|
| `fuzz_meta_negation` | play `通用:GREAT` (no [指定]) and fully negate it with `通用:网络链接异常`; final money equals never playing it |
| `fuzz_meta_immunity` | `Mor:夏日合宿` up, `通用:登上武道馆` from another player; the shielded player pays nothing |
| `fuzz_meta_commutativity` | two `AG:（摩卡）0.5倍速` (x0.5 each) in swapped field order give the same rent |
| `fuzz_meta_monotonicity` | adding `AG:Y.O.L.O` (+1d4) never shortens a move |
| `fuzz_meta_seat_rotation` | skillless symmetric board, character list rotated; sorted (money, pos) multiset is unchanged |
| `fuzz_determinism` | same seed + same answer sequence -> identical `Match::save()` JSON |
| `fuzz_save_restore` | `Match::save()` mid-run, `Match::restore`, continue with the same answers -> identical final state |

### Documented exclusions

* **Instance-level card-zone disjointness** is infeasible: the engine stores
  cards as string ids without instance identity (`Hidden.hand` is
  `Vec<String>`), so two copies of one id may legally sit in different zones.
  The check covers id validity (every id is in the manifest or a `skill:` /
  `TEST:` id) and field-uid uniqueness instead.
* **`bankrupt` and auction escrow** windows are skipped by the money ledger
  (see finding 1).
* **Unbounded keyed state** (`skill.*`, `want_to_grab_passer`, ...) may use
  `-1` as a "no target" sentinel and is not checked for non-negativity; only
  the rulebook-bounded counters (stay / stun / exile / fire / crystals /
  tokens) are.

## Unintended interactions

Q4 -- unintended card / skill / tile interactions. Harness:
`crates/game-rules/tests/q4_unintended.rs` plus the modules under
`crates/game-rules/tests/q4/` (`snap` whole-`World` snapshot + surface map,
`single` footprint audit, `pair` non-interference, `trig` trigger scope,
`expiry` expiry / leak, `names` shared-name registry, `seat` bystander /
seat invariance). Findings are filed as `#[ignore = "DISCREPANCY: unintended:
..."]` tests in `crates/game-rules/tests/rb_unintended_found.rs`; the default
`sweep()` stays green by recognising their signatures in
`q4::KNOWN_FINDINGS`, the same way `fuzz::KNOWN_FINDINGS` does.

Black-box: `rules/cards/`, `rules/skills/`, `rules/fixtures/` and
`target/scratch/tainted/` were never opened. Rule ids come from the shipped
manifest (`ruleset().cards()`), footprints from `docs/rulebook/footprints.json`,
rule text from `target/scratch/rb/*.md`.

### Method and sample sizes

| check | method | default sample | seeds |
|---|---|---|---|
| 1. single-rule footprint | arrange a rich board (deeds / houses / money / positions / statuses / fire, no other field cards, no bound skills), snapshot the whole `World`, activate one rule, snapshot again, diff. Every non-bookkeeping change must sit inside the rule's declared `writes` (plus the play cost of the press). | 53 rules x 2 = 106 activations (stride over the 249 rule ids) | `Q4_SEEDS`, default 2 |
| 2. pairwise non-interference | for a disjoint-surface pair `(A, B)` (no shared `writes u reads` key), run B alone and A-then-B from the same arrangement seed and the same deterministic answers; require `delta(B after A) == delta(B alone)` on B's declared surfaces. | 40 pairs, 14 live (B changed something) | 1 per pair |
| 3. trigger-scope | place the rule on P0, run P0's turn and P1's turn as separate windows, collect every event that names the rule (`MatchEvent::card` / `Arg::Card` / `log.hook_fire`). "your turn" rules must not fire in P1's window. | 40 rules x 2 windows | 1 per rule |
| 4. expiry / leak | same arrangement twice -- with and without the activation -- push both through a full turn lap (every player's end / start boundary), require the rule's turn-scoped leftovers to match the control. | 36 rules x 2 = 72 rule-seeds | 1 per rule-seed |
| 5. shared-name registry | diff world state per action and log every keyed state / token / mark / prop name written; fold in the single-rule audit's names. Report collisions (two rules with disjoint footprints writing one name), write-never-read, and near-duplicates (edit distance 1, normalised equality, containment). | 50 rules + the 106 single-rule activations | 1 |
| 6. bystander / seat invariance | symmetric 3-player board, A and B act, C is the bystander; rotate the seat labels and require C's personal delta to match. | 20 pairs | 1 per pair |

Widen with `Q4_SAMPLE=0 Q4_SEEDS=4 Q4_PAIRS=120 cargo test -p game-rules
--test q4_unintended q4_soak -- --ignored --nocapture`. Default runtime
**~35 s** for the whole file (the ~2 minute band includes compile).

### Findings

| rule(s) | field | repro test | severity |
|---|---|---|---|
| (engine play-ctx) `TurnCtx.play_from_hand` | turn ctx | `rb_unintended_found::play_from_hand_flag_leaks_past_the_turn` | **fixed** (scoped to the play) |
| `Mujica:心の雨` | `turn.abnormal` | `rb_unintended_found::kokoro_no_ame_leaves_turn_abnormal` | **fixed** (zeroed at turn start) |
| `通用:登上武道馆` | `targeted[]` | `rb_unintended_found::budokan_leaves_targeted_counters` | **fixed** (the repro's turn lap now rolls first; `targeted[i]` was already zeroed at `i`'s turn start) |
| `PP:初次演出事故` | token names (`skillBlock:Pastel✽Palettes` vs `skillBlock:PP2`) | `rb_unintended_found::stage_accident_writes_two_skillblock_names` | medium |
| `HHW:爱心义演` | `state:charity_extra_total`, `state:charity_show_turn` | `rb_unintended_found::charity_writes_undeclared_state` | low |
| `RAS:EXIST` | `state:exist_used` | `rb_unintended_found::exist_writes_undeclared_state` | low |

What they are:

* **`turn.play_from_hand` is never cleared.** `play_from_hand` sets
  `TurnCtx.play_from_hand = true` and only a nested `ctx::play_card` clears
  it. The flag is still `true` after the play ends and after a full turn
  lap. "If this card was played from somewhere other than the hand" reads
  it, so every later reader sees the stale `true` and takes the wrong
  branch -- an unintended coupling between every hand press and every
  `play_from_hand` reader. Seen after playing 13 distinct rules in the sweep.
* **`Mujica:心の雨` leaves `turn.abnormal`.** A one-shot `[手]` play with no
  turn-start / turn-end hook; the [停留] it grants is an abnormal-move
  effect so the engine stamps `turn.abnormal`, and the stamp survives a
  full turn lap (documented as "a new turn starts them all at 0").
* **`通用:登上武道馆` leaves `targeted[]`.** The multi-target pay designates
  every other player; `World::targeted` ("times other players' cards
  targeted it since its own turn last started") is still `[0, 1, 1]` after
  a full turn lap, so every player's own turn-start reset failed to zero
  it.
* **`PP:初次演出事故` writes two spellings of one token.** The Pastel Palettes
  (2)-skill suppression is recorded under both `skillBlock:Pastel✽Palettes`
  (the band name, star glyph included) and `skillBlock:PP2` (an
  abbreviation). A later reader that looks up one of the two misses the
  other; a third spelling (`Pastel*Palettes` / `PastelPalettes`) is one
  rename away. This is the P-star-P fan-name variant the registry is for.
* **`HHW:爱心义演` / `RAS:EXIST` write undeclared keyed state.** Each stamps
  a scratch slot (`charity_extra_total` + `charity_show_turn`,
  `exist_used`) that its `footprints.json` `writes` list does not declare.
  The names are not in any other rule's vocabulary either, so they are
  typo-shaped: nothing can legally read them back.

Disappeared mid-run (fixed by the concurrent engine / card work or by a
driver artifact the audit now absorbs, noted so the next sweep does not
re-file them):

* `PPP:[衍生]Pipopa` briefly showed a `crystals` write on
  `PPP:[衍生]Popipapapipopa` -- that was the new instance's initial `0`,
  not a crystal write; the audit's placement-cost filter now absorbs it.
* `skill:北泽育美:全垒打！` briefly showed a standing croquette mark as a
  leak. The skill text is "every turn generate a croquette on the meat
  shop", so the mark is a standing resource; the expiry check now skips
  marks for rules with an ongoing hook.
* `PPP:献给远方的你` briefly showed a `money` + `tile.owner` stray. The
  driver was pressing "buy" on `ask.buy` after the forced settle; the
  driver now declines `ask.buy` / `ask.build` / counteract windows and only
  takes `ask.force_buy`.

### Shared-name registry

17 distinct keyed names written across the default sweep:

| name | written by |
|---|---|
| `fprop:handLimitDelta` | `PP:不要背负期待`, `PP:练习生解密指南` |
| `mark` | `Mor:高贵的微蓝` |
| `state:charity_extra_total` | `HHW:爱心义演` |
| `state:charity_show_turn` | `HHW:爱心义演` |
| `state:exile` | `Sumimi:一人两个甜甜圈` |
| `state:exileTo` | `Sumimi:一人两个甜甜圈` |
| `state:exist_used` | `RAS:EXIST` |
| `state:fate_hit` | `CRYCHIC:一起演奏音乐的命运共同体` |
| `state:jennifer_user` | `PP:找回珍妮弗` |
| `state:no_expectation_stacks` | `PP:不要背负期待` |
| `state:skillState` | `Mujica:欢迎来到ave mujica的世界` |
| `state:stay` | `MyGO:那天的雨` |

* **Collisions (unrelated rules writing one name):** none in the default
  sample. `fprop:handLimitDelta` is shared by two `PP:` cards, but both
  declare `hand.limit`, so the share is on-surface.
* **Write-never-read:** `state:charity_*`, `state:exist_used` (the two
  footprint gaps above). The engine-owned keys (`state:exile`,
  `state:exileTo`, `state:skillState`, `state:stay`) are a frozen wire
  contract and are excluded from the typo scan.
* **Near-duplicates:** the `skillBlock:Pastel✽Palettes` /
  `skillBlock:PP2` pair (filed above). `state:exile` vs `state:exileTo` is
  an engine key pair, not a content typo.

### Allowlist (engine bookkeeping)

A write is not a rule write when it is one of:

* derived mirrors recomputed every tick: `roll:P*`, `assets:P*`,
  `hand_n:P*`, `draw_n:P*`, `discard_pub:P*`;
* movement scratch the engine writes on every walk: `state:P*:lastWalk`;
* event-deck reshuffles (turn start draws an event): `event_deck`,
  `event_discard`, `event_removed`, `st.event_active`;
* turn bookkeeping that grows because the harness acted: `turn.played`,
  `turn.player`, `turn.main_moved`, `turn.main_steps`, `turn.turn_rolls`,
  `turn.turn_snap`, `turn.turn_start_pos`, `next_turn_pending`;
* engine status ticks: a `stay` / `stun` / `stunStart` / `exile` item losing
  one layer to the turn-boundary tick (value drops, `expires` flips to
  `Some(TurnEnd)` / `Some(TurnStart)`) -- `snap::is_engine_tick`;
* the play cost of the press: the activated card's own id leaving `hand`
  and appearing in `field` / `discard.pile`, and a newly placed instance's
  `crystals` starting at `0`;
* `turn.play_from_hand` flipping to `true` at the press (the *stale* value
  after the turn is the filed finding, not the flip itself);
* `turn.abnormal` growing when the same activation writes `status.stay` /
  `status.stun` (the engine's record of the status application);
* `scheduled` growing for a rule whose own triggers include `AtEnd` (its
  callback queue is its effect mechanism).

### Coverage limits

* Trigger-scope observed only 2 `log.hook_fire` events in the default
  window (most hooks need a richer chain to fire). The own-turn /
  other-turn split is in place; widen with `Q4_SAMPLE` to see more.
* Pairwise: 14 of 40 pairs were live (B changed something). The rest had
  B's press refused on the arranged board; they are counted but not
  asserted.
* Single-rule: a rule whose effect is entirely inside a prompt we decline
  (an optional clause, a counteract window) is under-observed. The audit
  is an over-fire check, not an under-fire check.
* The driver declines `ask.buy` / `ask.build` / counteract windows and
  takes `ask.force_buy`, so a settle that *forces* a buy still shows up as
  a `money` + `tile.owner` write and is correctly attributed to the rule.

## Rulebook doc (2026-10-06)

Clause-by-clause check of the **core rulebook** against the engine. Source:
the live Google Doc 规则书 tab, snapshotted as
[rulebook-doc.md](rulebook-doc.md) (fetched 2026-10-06). The engine's cited
copy `data/rules.txt` has the same prose but stubs out the 时点流程 /
支付阶段 tables; see [TEST-FINDINGS.md](TEST-FINDINGS.md) §Rulebook doc check
for the diff.

Status codes here are for **rulebook** clauses, not card clauses:
**ok** = implemented as written · **diff** = implemented differently (say
how) · **miss** = missing · **na** = not applicable (UI / social / card
domain) · **todo** = `TODO(规则书)` in the tree (the book does not say).
Tests are `rb_rulebook.rs` unless another suite is named. Card effects
override the rulebook (特别注意 3), so a card bending a clause is not a hit.

### 专有名词 -- 资金与格子 (11)

| clause | meaning | status | evidence / tests |
|---|---|---|---|
| `G:场地#1` | the play area holding deeds, cards, piles | **na** | UI / social |
| `G:获得#1` | 「[获得]X」 prints money | **ok** | `play.rs` `money` `gain` |
| `G:消耗#1` | 「[消耗]X」 deletes money | **ok** | `play.rs` `money` `lose` |
| `G:支付#1` | 「A[支付]BX」 moves X from A to B | **ok** | `play.rs` `money`; `rb_money` |
| `G:结算#1` | settle runs every effect on the tile | **ok** | `settle_at` → `settle_tile` → tile rule instances |
| `G:破产#1` | cannot pay (incl. mortgage) → liquidate, funds to the creditor | **ok** | `play.rs` `raise_funds`/`bankrupt`; `rb_rulebook::b01` |
| `G:存活#1` | not bankrupt and not left | **ok** | `World::out` |
| `G:地产商#1` | one agent per colour | **ok** | `tile:agent`; board `group` (TODO on 「同色」) |
| `G:可购买格子#1` | not CiRCLE / cafe / 江户川 / 流星堂 / agent | **ok** | `TileData::is_buyable` |
| `G:拍卖#1` | all alive who can spend; first highest wins; min 100, +100 | **ok** | `mod.rs` auction handler `min = bid<=0 ? 100 : bid+100`; `rb_rulebook::b03` |
| `G:CiRCLE奖励#1` | gain 2000 or draw 1 | **ok** | `play.rs` `circle_reward`; `rb_rulebook::s04`, `game-core play/tests::passing_circle_offers_money_or_a_card` |

### 专有名词 -- 卡与技能 (12)

Card domain (the `rb_<group>` suites). The engine-side hooks these name are
present: `[指定]` (`Effect.target` / `targeted`), `[移除]` (`Dest::Banished`),
`[主动]` (`use_skill`), `[手]` (`play_from_hand` gate), `[反击]`
(`hand_counteractions` / `rb_chain`), `[持续]` (`FieldCard`),
`[特]` (`deckBeforeGame` / `deckAtGameStart`), `[限]` (`cant_play`).

### 专有名词 -- 移动 (15)

| clause | meaning | status | evidence / tests |
|---|---|---|---|
| `M:主要移动#1` | the move phase's move | **ok** | `play.rs` `main_move`; `rb_rulebook::gf07` |
| `M:结算#1` | execute the tile's effects | **ok** | `settle_at` |
| `M:移动起点#1` | the tile before the move | **ok** | `MoveCtx.from` |
| `M:移动终点#1` | the tile after the move | **ok** | `MoveCtx.to` |
| `M:路径#1` | excludes the start, includes the end; same start/end includes it | **diff** | walk excludes the start and includes the end (`rb_rulebook::t05`); a same-tile teleport now fires [经过] then [重叠] then [结算] at the target (`B41`, `t06b`); a 0-move fires [重叠] only (`E14`, `t06c`) -- TODO(规则书) whether [经过] also fires there |
| `M:经过#1` | fire when X is on the path | **ok** | `passBefore`/`passTile`/`pass` per step |
| `M:强制移动#1` | a forced move | **ok** | `card_move` / `MoveCtx` |
| `M:强制停下#1` | a forced stop (doc's definition is truncated) | **ok** | `MoveCtx.stop_at` / `plan::set_stop_at` |
| `M:传送#1` | path is just the end | **ok** | `play.rs` `teleport`; `rb_rulebook::t04` |
| `M:无法移动#1` | main move ends in place and does not settle; non-teleports cancelled | **ok** | `st.skip_move`; `rb_rulebook::m04` |
| `M:停留#1` | [无法移动]; stacks; one layer off at each turn end | **ok** | `give_stay` + `Tick::TurnEnd`; `rb_rulebook::m02`, `t02` |
| `M:晕眩#1` | [无法移动] + no 主动 / [手] / pay-receive; stacks; one off at turn end | **ok** | `give_stun`; `rb_rulebook::m03`, `t01`; `rb_cross_status::s02`/`s03` |
| `M:除外#1` | [无法移动] + no [指定] / 主动 / [手] / pay-receive; off-board; stacks; one off at turn **start**; skills and [持续] stop; teleports back to the written tile | **ok** | `give_exile` / `turn_start`; `rb_rulebook::t03`/`t04`; `rb_cross_status::s01` |
| `M:不可阻挡#1` | ignores stay, no new stun/exile layers, layers still drop, may refuse teleport/forced move/stop | **ok** | `key::UNSTOPPABLE`; `rb_cross_move::m13`/`m30`; `rb_gap_window::g23`/`g24` |
| `M:异常移动效果#1` | teleport, stay, stun, exile, forced move, forced stop, reverse | **ok** | `abnormalGuard` / `rb_guards` |

### 游戏中各模式的开始规则和胜利条件

| clause | meaning | status | evidence / tests |
|---|---|---|---|
| `E:娱乐#1` | 3-10 players | **na** | match setup (the harness asserts 2..=10) |
| `E:娱乐#2` | each rolls 1d20, descending; ties re-roll (bold) | **ok** | `setup.rs` `roll_order`; `game-core engine.rs::ranked_games_run_the_ban_phase` |
| `E:娱乐#3` | pick distinct characters; 10-card deck from general + band + exclusive; no [衍生] | **diff** | pick/deck is `setup.rs`; the 10-card / [衍生] rule is card-domain (`rb_<group>`) |
| `E:娱乐#4` | each picks a distinct [场地] | **na** | UI / social |
| `E:娱乐#5` | may end by mutual agreement | **ok** | `Match::end_vote`; `game-core engine.rs::unanimous_vote_ends_the_match` |
| `R:正规#1` | 5-6 players | **na** | match setup |
| `R:正规#2` | each may ban one character before picking | **ok** | `setup.rs` `begin_ban`/`do_ban` |
| `R:正规#3` | ban reverse order, pick forward order | **ok** | `setup.rs` |
| `R:正规#4` | last alive wins | **ok** | `check_game_over` `finish("last")`; `rb_rulebook::w01` |

### 游戏流程 (9)

| clause | meaning | status | evidence / tests |
|---|---|---|---|
| `F:流程#1` | start on CiRCLE, 10000, draw 2; one mulligan | **ok** | `begin_play`/`opening`; `rb_rulebook::gf01`/`gf02` |
| `F:流程#2` | hand limit 5; discard down; non-[持续] used cards to discard; reshuffle | **ok** | `hand_limit()` base 5; `draw`/`discard`; `rb_rulebook::gf03`/`gf04` |
| `F:流程#3` | 开始/运营/移动/结束; next player after a turn (bold) | **ok** | `stage` + `next_turn` |
| `F:流程#4` | non-[反击] [手] only in 运营 | **ok** | `play.rs` `step == stage::OPS` gate |
| `F:流程#5` | mortgage at 运营 or when short; +50% of price (bold) | **ok** | `mortgage_value = price/2`; `rb_rulebook::gf05`, `game-core play/tests::short_of_cash_mortgages_then_pays` |
| `F:流程#6` | redeem at 运营; -60% of price (bold, 60% red) | **ok** | `redeem_cost = round(price*0.6)`; `rb_rulebook::gf06` |
| `F:流程#7` | flip the deed in a non-automated game | **na** | UI |
| `F:流程#8` | 1 [主要移动] per turn; base 1d20 clockwise (bold); settle at the end | **ok** | `main_move` + `main_moved`; `rb_rulebook::gf07`/`t07` |
| `F:流程#9` | bankruptcy / leave: clear pieces+cards+effects; deeds ownerless (bold, red); auction 3 | **ok** | `remove_from_game`/`auction_leftovers`; `rb_rulebook::b01`/`b02` |

### 其他规则注意事项 (6)

| clause | meaning | status | evidence / tests |
|---|---|---|---|
| `O:其他#1.1` | [经过] along the path in order; recompute the path if the end changes | **ok** | walk loop re-reads `plan` per step; `rb_cross_move::m17_lock_redirects_destination` |
| `O:其他#1.2` | a skill/card [主要移动] states whether it settles | **ok** | `MoveCtx.resolve`; `after_walk` |
| `O:其他#2` | designate many: activator first, action order | **ok** | `hand_counteractions` / card domain |
| `O:其他#3` | multiple [反击]: next player first; one timing per round | **ok** | `rb_chain` (TEST-FINDINGS §10) |
| `O:其他#4` | non-optional effects must fire | **na** | card domain |
| `O:其他#5` | band/char effects judged by the current cards | **ok** | `bind_skills` |
| `O:其他#6` | mortgage / redeem / forced purchase unaffected by effects | **ok** | all three move money outside `money()`; `rb_rulebook::o06`, `s11` |

### 基础[结算]规则 (11)

| clause | meaning | status | evidence / tests |
|---|---|---|---|
| `S:结算#1` | CiRCLE + 江户川 draw 1 | **ok** | `tile:circle`/`tile:edogawa` settle; `rb_rulebook::s01`/`s02` |
| `S:结算#1.1` | [经过]CiRCLE with start ≠ CiRCLE → [CiRCLE奖励] | **ok** | enforced in `play.rs::circle_reward` (the reward step both the tile body's Pass entry and the walk's fallback call); `rb_rulebook::s05`, `s05b` |
| `S:结算#2` | cafe + 流星堂 draw 1 + 1 event | **ok** | `tile:event`; `rb_rulebook::s03` |
| `S:结算#2.1` | the event is public, not in hand, immediate | **ok** | `draw_event` |
| `S:结算#2.2` | event to the event discard; reshuffle when empty | **ok** | `setup_event_deck` / `draw_event` |
| `S:结算#3` | ownerless: may pay price+houses, gain the deed (bold) | **ok** | `offer_buy`/`buy_price`; `rb_rulebook::s06` |
| `S:结算#5.1` | own, unmortgaged: may pay the build cost to upgrade (bold); level cap; cannot fund by mortgaging that deed | **ok** | `build`/`why_not_build_on` (`buildMax`, RiNG cap 0); the mortgage exclusion is emergent (a mortgaged deed refuses the build); `rb_rulebook::s08`/`s09` |
| `S:结算#5.2` | own, mortgaged: no effect | **ok** | `property.rs` own branch; `rb_rulebook::s07` |
| `S:结算#6.1` | other's, unmortgaged: pay the current-level rent (bold) | **ok** | `pay_rent` rent table; `game-core play/tests::rent_follows_the_rent_table` |
| `S:结算#6.2` | other's, mortgaged: 2×(price+houses) force-buy, stays mortgaged (bold, red); price and payee unaffected | **ok** | `offer_force_buy` moves money directly; `rb_rulebook::s10`/`s11`, `game-core play/tests::mortgaged_land_can_be_force_bought_at_double` |
| `S:结算#7` | agent: if all same-colour are others' → half-charge each front-to-back (ceil 10); else settle one | **ok** | `agent_landing`; `half_ceil10` in `pay_rent`; `rb_rulebook::s12`, `game-core play/tests::agent_charges_half_rent_...` (TODO on 「同色」/`extraColor`) |

### 时点流程 -- 开始游戏 (8)

| window | status | evidence |
|---|---|---|
| 1 禁用角色 | **ok** | `setup.rs` `begin_ban` (正规 only) |
| 2 选角色 | **ok** | `setup.rs` `begin_pick` |
| 3 选卡 | **diff** | `setup.rs` `submit_deck`; the 10-card / [衍生] rule is card-domain |
| 4 游戏开始前 [特] | **ok** | `deckBeforeGame` |
| 5 初始抽卡 2 | **ok** | `opening` |
| 6 初始重抽 | **ok** | `mulligan` |
| 7 游戏开始时 [特] | **ok** | `deckAtGameStart` |
| 8 第一回合开始 | **ok** | `next_turn` |

### 时点流程 -- 回合流程 (11)

| window | status | evidence |
|---|---|---|
| 1 回合开始前 | **diff** | no trigger; the 行动阶段 reading (除外 decay) runs inside `turn_start` after `turnStartBefore` |
| 2 回合开始时 | **ok** | `turnStart` |
| 3 回合开始后 | **miss** | no `turnStartAfter` kind |
| 4 进入运营阶段 | **ok** | `stage::OPS` |
| 5.1 使用手卡 | **ok** | `play` command |
| 5.2 抵押地契 | **ok** | `mortgage` command |
| 5.3 赎回地契 | **ok** | `redeem` command |
| 6 进入移动阶段 | **ok** | `stage::MOVE` |
| 7 执行[主要移动] | **ok** | `main_move` |
| 8 执行[结算] | **ok** | `settle_at` |
| 9/10/11 回合结束前/时/后 | **ok** | `endTurnBefore`/`turnEndBefore`, `tick_state(TurnEnd)`+`turnEnd`, `turnEndAfter`/`endTurnAfter` |

### 时点流程 -- 行动阶段 (19)

| # | window | status | engine kind |
|---|---|---|---|
| 1 | 回合开始前 (除外 -1) | **diff** | `turnStartBefore` then the exile decrement (between 1 and 2, not inside 1) |
| 2 | 回合开始时 | **ok** | `turnStart` |
| 3 | 回合开始后 | **miss** | none |
| 4 | 经营/出牌阶段前 (晕眩 skips) | **diff** | the stun check inside `turn_start` (no trigger) |
| 5 | 经营/出牌阶段 | **ok** | `stage::OPS` |
| 6 | 经营/出牌阶段后 | **miss** | none |
| 7 | 主要移动阶段前 (停留 skips; one move) | **diff** | `skip_move` + `main_moved`, both checked early (no trigger) |
| 8 | 掷骰前 | **ok** | `roll` (`value = -1`) |
| 9 | 掷骰时 | **diff** | `rollPlan` + an un-raised table roll |
| 10 | 掷骰后 | **ok** | `rollAfter` + `moveRoll` |
| 11 | 移动前 | **miss** | none |
| 12 | 移动时/[经过] | **ok** | `passBefore`/`passTile`/`pass` per step; a teleport fires at the end **including onto its own tile** (`B41`; `rb_rulebook::t06b`). TODO(规则书): whether a 0-step move also fires [经过] -- `E14` names only [重叠] there (`t06c`) |
| 13 | 移动后/[重叠] | **ok** | `passPlayer` at the end of a walk (`E13`, `t06`), on a teleport's target (`B41`, `t06b`) and on a 0-step move (`E14`, `t06c`) |
| 14 | 结算前 | **ok** | `settleBefore` |
| 15 | 结算 | **ok** | `settle` + `settleBody` |
| 16 | 结算后 | **ok** | `settleAfter` |
| 17 | 结束阶段前 | **ok** | `endTurnBefore` / `turnEndBefore` |
| 18 | 结束阶段 (晕眩/停留 -1) | **ok** | `tick_state(Tick::TurnEnd)` |
| 19 | 结束阶段后 | **ok** | `turnEndAfter` / `endTurnAfter` |

### 时点流程 -- 支付阶段 (7)

| # | window | status | engine kind |
|---|---|---|---|
| 1 | 支付计算前 | **diff** | folded into `effect`, which also carries window 5's cancel/retarget power |
| 2 | 支付计算时 (flat add/sub) | **ok** | `payAdd` |
| 3 | 支付计算后 | **miss** | none between `payAdd` and `payMul` |
| 4 | 支付前 (halve/double) | **ok** | `payMul` + `scale_settle_payment` |
| 5 | 支付时 (pay / 无需失去 / 取消 / 更改目标) | **diff** | split across `effect` + `payChoose`/`payAt` (unnamed) + `pay` |
| 6 | 支付后 | **ok** | `payAfter` |
| 7 | 资金变动 | **diff** | `paid`, payer-loss only; a distinct kind from `payAfter` despite the doc's 「合并到[支付后]?」 |

### 标志物解释 (7)

| marker | status | note |
|---|---|---|
| CP点 | **na** | UI marker (`TileMark`) |
| 火罐 | **ok** | `key::FIRE` + its `max` cap (「不能超过持有上限」) |
| 雨伞 = [停留] | **na** | UI |
| 弹簧 = [晕眩] | **na** | UI |
| 奇迹水晶 | **ok** | `FieldCard::crystals` |
| P✽P粉丝 | **ok** | `token("P✽P粉丝")` |
| 抹茶巴菲 | **na** | UI |

### Tallies

| group | clauses | ok | diff | miss | na | todo/open |
|---|---|---|---|---|---|---|
| 专有名词 资金与格子 | 11 | 10 | 0 | 0 | 1 | 1 (agent 「同色」) |
| 专有名词 卡与技能 | 12 | 12 | 0 | 0 | 0 | 0 (card domain) |
| 专有名词 移动 | 15 | 13 | 1 | 0 | 0 | 1 (same-tile path) |
| 开始规则/胜利 | 9 | 6 | 1 | 0 | 2 | 0 |
| 游戏流程 | 9 | 8 | 0 | 0 | 1 | 0 |
| 其他规则注意事项 | 6 | 5 | 0 | 0 | 1 | 0 |
| 基础[结算] | 11 | 10 | 0 | 1 | 0 | 1 (S1.1) |
| 时点 开始游戏 | 8 | 7 | 1 | 0 | 0 | 0 |
| 时点 回合流程 | 11 | 9 | 1 | 1 | 0 | 0 |
| 时点 行动阶段 | 19 | 12 | 5 | 2 | 0 | 1 (13 DISCREPANCY) |
| 时点 支付阶段 | 7 | 4 | 2 | 1 | 0 | 2 (order + 资金变动) |
| 标志物 | 7 | 3 | 0 | 0 | 4 | 0 |
| **total** | **125** | **99** | **11** | **5** | **9** | **7** |

`todo/open` counts clauses with a `TODO(规则书)` in the tree or an open §6
ruling; those are already counted in ok/diff/miss. The five `miss` rows are
the four unnamed timing windows (行动阶段 3/6/11, 支付阶段 3) plus
基础[结算] 1.1's start-tile check. New tests live in
`crates/game-rules/tests/rb_rulebook.rs` (33 tests, 2 `#[ignore]` as
DISCREPANCY).