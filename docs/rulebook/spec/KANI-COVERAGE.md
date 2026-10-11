# Kani host-op coverage (cards / skills / events / tiles / marks)

Generated 2026-10-11 (HOST-OPS pass) from:

* **op usage** — `ctx::` paths grepped from each guest source under
  `rules/cards`, `rules/skills`, `rules/events`, `rules/tiles`, `rules/tile_marks`
  (row inventory: card_id → path → ops, same shape as `cov_rows.txt`).
* **modeled ops** — `Op::*` arms in `rules/kani/src/host.rs::dispatch` that do
  **not** fall through to `unmodeled()`. A fall-through is a loud fail
  (`kani::assert(false, "unmodeled host op")` / native `panic!`), never a silent zero.

**Definition.** A row is *fully modeled* when every host op its body reaches has an
explicit `dispatch` arm. Types and constants (`Dest::Field`, `CardPile::Discard`,
`state::TURN_END`, …) are arguments, not ops, and are excluded.

**Protocol-level stubs.** Some modeled arms are structure-only (return `0`) for
engine-scheduling surfaces (move plan, offers / buy / build, multi-turn settle).
Those count as *modeled* here (they never hit `unmodeled`), but a clause whose proof
needs their real semantics is **PROTOCOL-LEVEL** — see `KANI.md` §6 and
`KANI-FOUNDATION.md` §Not-Kani-expressible. Do not fake it with a silent zero.

## Summary

| kind | fully modeled | total | blocked |
|---|---:|---:|---:|
| card | 183 | 183 | 0 |
| skill | 66 | 66 | 0 |
| event | 28 | 28 | 0 |
| tile | 6 | 6 | 0 |
| mark | 1 | 1 | 0 |
| **all** | **284** | **284** | **0** |

### By unit

| unit | fully modeled | total | blocked |
|---|---:|---:|---:|
| `ag` | 15 | 15 | 0 |
| `crychic` | 15 | 15 | 0 |
| `events` | 28 | 28 | 0 |
| `general` | 14 | 14 | 0 |
| `hhw` | 15 | 15 | 0 |
| `mor` | 15 | 15 | 0 |
| `mujica` | 15 | 15 | 0 |
| `mygo` | 15 | 15 | 0 |
| `pp` | 19 | 19 | 0 |
| `ppp` | 17 | 17 | 0 |
| `r` | 17 | 17 | 0 |
| `ras` | 15 | 15 | 0 |
| `skills_bands` | 12 | 12 | 0 |
| `skills_characters` | 54 | 54 | 0 |
| `sumimi` | 11 | 11 | 0 |
| `tile_marks` | 1 | 1 | 0 |
| `tiles` | 6 | 6 | 0 |

### Unmodeled ops that actually appear in guest bodies

**None.** The 2026-10-11 HOST-OPS pass modeled the full previous fall-through
list:

| `Op` | model source (faithful to) |
|---|---|
| `rent_of` | `WorldOps::rent_of` + `purchase::table_rent` (RiNG = `price`) |
| `tile_at` / `set_tile_at` | `WorldOps::tile_at` / `set_tile_at` (field instance) |
| `trig_price`, `trig_seller`, `trig_buy_kind` | `TriggerBuy` (hostfns 1886–1900) |
| `trig_deal_owner`, `trig_deal_houses`, `trig_deal_mortgaged` | `TriggerBuy::deal_*` |
| `trig_move_kind/resolve/tag/main/dir/remaining/from/total/roll` | `TriggerMove` sentinels (hostfns 1830–1880); `trig_move_main` is `Trigger::main` |
| `trig_roll_source` | `RollSource::as_i32` |
| `trig_pay_is_rent` | `TriggerPay::is_rent` |

Full host fall-through list: **empty** (every `Op` variant has an explicit arm).
Protocol stubs (structure-only, return 0) remain for engine-scheduling
surfaces — offers / buy / build windows, multi-turn settle, `pay_rent`,
`card_*` purchase commands, `msg_*`. A clause that needs their *real*
scheduling semantics is still **PROTOCOL-LEVEL** (KANI.md §6). Do not fake it
with a silent zero on a surface the proof needs.

## Full table

Columns: `unit | id | ops used | all ops modeled? | blocked-by`.
`ops used` are the `ctx::` paths as written in the guest (module paths kept).
`blocked-by` names the unmodeled `Op`s those paths resolve to.

| unit | id | ops used | all ops modeled? | blocked-by |
|---|---|---|:---:|---|
| `ag` | `AG:(兰) 像往常一样` | `abnormal_count,before_turn_end,effect::has,log,set_slot,slot,state::set,teleport_to,turn_key,turn_snap` | yes | — |
| `ag` | `AG:ONE OF US` | `ask_player,ask_tile,in_band,is_shop,log,others,owned_count,owned_tiles,place_card,player_out,set_dest,set_owner,set_slot,slot,target_tile,tile_owner,transfer` | yes | — |
| `ag` | `AG:Y.O.L.O` | `log,roll` | yes | — |
| `ag` | `AG:刻入天穹傲岸的烈光` | `owned_tiles,player_out,players_on,tile_price,transfer` | yes | — |
| `ag` | `AG:即使夕阳落山` | `add_house,ask_tile,build_cost,houses_of,log,others,owned_tiles,round_no,target_all,tile_price,transfer` | yes | — |
| `ag` | `AG:商店街的青梅竹马` | `cant_move,card_move,is_shop,log,roll,tile_count,tile_named,tile_owner` | yes | — |
| `ag` | `AG:回家的路上绕个道` | `ask_pick,at_turn_end,cant_move,log,plan,plan::dir,plan::set_base_dice,plan::set_parity,plan::set_reverse,plan::set_start,player_pos,set_tile_prop,state::set,tile_count,tile_forward,tile_named` | yes | — |
| `ag` | `AG:宣战布告` | `draw,effect::count,effect::hits,effect::tile,player_out,target,tile_owner,transfer` | yes | — |
| `ag` | `AG:无论是何种颜色的夕阳` | `add_to_deck_at,add_to_hand,ask_card,ask_pick,cards_in,draw,effect,fire,fire_max,gain,gain_fire,log,roll,take_card` | yes | — |
| `ag` | `AG:朝同一片天空迈进` | `add_to_deck,draw,gain,hand_size,log,take_from_hand,to_discard` | yes | — |
| `ag` | `AG:绯红之魂` | `add_crystals,ask_number,ask_yes,crystals,gain,is_placed,log,money_of,pay,place_card,player_out,set_crystals,set_dest` | yes | — |
| `ag` | `AG:（巴）商店街的救世主` | `acquire,gain,is_shop,log,money_of,mortgage_value,mortgaged_of,set_mortgaged,tile_owner,tile_price` | yes | — |
| `ag` | `AG:（摩卡）0.5倍速` | `decay,log,place_card,set_crystals,set_dest` | yes | — |
| `ag` | `AG:（绯玛丽）如果并非没问题` | `log` | yes | — |
| `ag` | `AG:（鸫）微小的『能做到』的事` | `ask_pick,cant_move,log,play_card,set_extreme` | yes | — |
| `crychic` | `CRYCHIC:一起演奏音乐的命运共同体` | `card_move,is_placed,log,neighbor,place_card,plan::set_resolve,plan::set_steps,player_out,player_pos,set_dest,set_slot,slot,tile_count,tile_owner` | yes | — |
| `crychic` | `CRYCHIC:主唱太拼命了` | `effect::has,log` | yes | — |
| `crychic` | `CRYCHIC:优雅的呐喊` | `at_next_turn_end,draw,log` | yes | — |
| `crychic` | `CRYCHIC:初演大成功` | `add_band_crystals,at_turn_end,give_stun,log,set_no_money_loss` | yes | — |
| `crychic` | `CRYCHIC:去唱卡拉ok吧` | `ask_pick,ask_yes,cant_move,log,roll,set_fixed_roll` | yes | — |
| `crychic` | `CRYCHIC:如果能一直持续下去...` | `add_band_crystals,band_crystals,gain,log,place_card,set_dest` | yes | — |
| `crychic` | `CRYCHIC:想要成为人类` | `abnormal_count,add_crystals,ask_number,crystals,log,place_card,set_crystals,set_dest,set_slot,slot` | yes | — |
| `crychic` | `CRYCHIC:想要抓住...` | `card_move,give_stay,log,state::get,state::set,trigger::player_id` | yes | — |
| `crychic` | `CRYCHIC:春日影` | `ask_pick,ask_player,ask_yes,build_cost,dist,do_move_roll,draw,hand_size,houses_of,log,money_of,mortgaged_of,others,owned_tiles,play_card,player_out,player_pos,take_from_hand,teleport_to,tile_count,tile_price,to_discard` | yes | — |
| `crychic` | `CRYCHIC:是我自己的问题` | `abnormal_count,ask_pick,dist,log,others,player_pos,teleport_to,tile_count,tile_forward` | yes | — |
| `crychic` | `CRYCHIC:（soyo）回到曾经` | `add_house,ask_pick,ask_tile,build_cost,cards_in,discard_from_hand,draw,gain,log,owned_tiles,pay,take_from_hand` | yes | — |
| `crychic` | `CRYCHIC:（灯）内心的呐喊` | `ask_number,ask_yes,deck_count,discard_size,dist,fire,gain,in_band,log,owned_tiles,place_card_on,player_pos,self_tile,set_dest,set_slot,shuffle_into_deck,slot,spend_fire,teleport_to,trigger::player_id` | yes | — |
| `crychic` | `CRYCHIC:（睦）从没有觉得...` | `add_band_crystals,add_to_deck,ask_card,ask_pick,band_crystals,band_skill,band_skills,cards_in,discard_from_hand,draw,invoke_skill,log,plan::set_start,set_dest,sweep_to_deck` | yes | — |
| `crychic` | `CRYCHIC:（祥子）带领着大家` | `cant_move,log,plan::add_follower,plan::set_pay_factor,player_count,player_pos,players_on` | yes | — |
| `crychic` | `CRYCHIC:（立希）即便比不上...` | `ask_yes,do_move_roll,log,roll,turn_rolls` | yes | — |
| `hhw` | `HHW:Happy, Lucky, Smile, Yeah！` | `cant_move,card_move,log,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_pos,roll,tile_count` | yes | — |
| `hhw` | `HHW:出发！后台之旅！` | `add_to_deck_at,ask_card,ask_yes,cards_in,deck_count,discard_size,gain,log,take_card,to_discard` | yes | — |
| `hhw` | `HHW:因为我一直相信着你` | `add_to_deck_at,add_to_hand,ask_card,ask_player,cards_in,discard_from_hand,log,others,pay,take_card` | yes | — |
| `hhw` | `HHW:微笑巡逻队` | `add_house,build_cost,can_build_on,card_offer_build,houses_of,is_buyable,log,money_of,mortgaged_of,owned_tiles,roll,tile_count` | yes | — |
| `hhw` | `HHW:梦幻的回礼` | `add_band_crystals,ask_tile,can_pay,log,mortgaged_of,player_out,rent_of,state::set,tile_count,tile_owner,transfer` | yes | — |
| `hhw` | `HHW:热气球演出` | `ask_tile,cant_move,card_move,log,plan::set_kind,plan::set_resolve,plan::set_teleport_to,roll_ask,tile_count` | yes | — |
| `hhw` | `HHW:爱心义演` | `find_card,is_circle,log,place_card,plan::set_extra_steps,prop,set_dest,set_prop,tile_at,tile_owner,turn_key` | yes | — |
| `hhw` | `HHW:笑容大游行` | `add_crystals,ask_yes,card_settle_at,crystals,decay,log,place_card,plan::set_settle_as_agent,self_tile,set_crystals,set_dest,set_self_tile,tile_named,trigger::player_id,trigger::set_cancelled,trigger::tile` | yes | — |
| `hhw` | `HHW:运动的天赋` | `crystals,decay,do_move_roll,gain,is_placed,log,place_card,plan::set_min_roll,player_out,set_crystals,set_dest,set_slot,slot` | yes | — |
| `hhw` | `HHW:黑衣人的补给` | `bump_mark,count_marks,field_instances,log,place_card_on,place_mark,self_uid,tile_at,tile_named,trigger::tile,unplace_at` | yes | — |
| `hhw` | `HHW:（kkr）前往笑容集结的地方！` | `log,pay,place_card_on,placed_tile,player_count,self_tile,set_dest,tile_named,transfer` | yes | — |
| `hhw` | `HHW:（美咲）` | `ask_player,card_move,fire,log,others,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_out,player_pos,players_on,spend_fire,tile_count,tile_forward,transfer` | yes | — |
| `hhw` | `HHW:（育美）` | `ask_tile,ask_yes,count_marks,draw,gain,gains_this_turn,gate,is_placed,log,place_card,place_mark,plan::set_stop_at,remove_marks,roll,self_uid,set_dest,tile_count` | yes | — |
| `hhw` | `HHW:（花音）Wacha Mocha 啪嗒进行曲` | `add_tok,ask_tile,is_placed,log,place_card,plan::set_kind,plan::set_resolve,plan::set_teleport_to,set_dest,set_tok,tile_named,tok,trigger::player_id` | yes | — |
| `hhw` | `HHW:（薰）怪盗hello happy` | `add_tok,ask_player,ask_yes,decay,gate,log,others,place_card,plan::clear_dice,plan::set_base_dice,plan::set_resolve,plan::set_reverse,plan::set_stop_at,roll,set_crystals,set_dest,set_slot,slot,target,turn_key,turn_player` | yes | — |
| `mor` | `Mor:你的光芒将照亮前路` | `cant_move,dist,log,plan::set_start,player_pos,tile_named` | yes | — |
| `mor` | `Mor:再次牵起手来` | `log,neighbor,pay,place_card,player_out,set_dest` | yes | — |
| `mor` | `Mor:勇气展翅高飞之时` | `buy_price,gain,is_agent,log,player_out,roll,tile_count,tile_owner,tile_price,transfer` | yes | — |
| `mor` | `Mor:夏日合宿` | `draw,inc_slot,log,place_card,set_dest,set_slot,slot` | yes | — |
| `mor` | `Mor:离心力，不为所动` | `log,place_card,set_dest` | yes | — |
| `mor` | `Mor:秘密与青春的虹彩` | `character_is,log` | yes | — |
| `mor` | `Mor:纯真振翅` | `card_move,log,plan::dir,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_out,teleport_to,tile_count,tile_steps_ahead` | yes | — |
| `mor` | `Mor:蝴蝶飞舞的星月夜` | `add_crystals,ask_number,ask_yes,can_pay,crystals,decay,do_move_roll,fixed_roll,gain,is_circle,is_placed,log,money_of,pay,place_card,player_out,set_dest` | yes | — |
| `mor` | `Mor:迷茫之蝶们的三全音` | `add_crystals,effect::has,gain_fixed,is_placed,log,pay,place_card,set_crystals,set_dest,set_slot,slot` | yes | — |
| `mor` | `Mor:高贵的微蓝` | `card_settle_at,log,place_mark,player_pos,self_uid,tile_owner,tile_price` | yes | — |
| `mor` | `Mor:（NNM）稍微努力了一下` | `add_crystals,add_tok,ask_number,ask_pick,before_turn_end,cards_in,crystals,discard_from_hand,draw,gain,is_placed,log,place_card,placed_cards,play_card,set_dest,state::get,state::set,tok,tok_names` | yes | — |
| `mor` | `Mor:（Rui）正论恶魔` | `log,others,player_out,set_slot,transfer` | yes | — |
| `mor` | `Mor:（toko）` | `ask_pick,buy_price,log,mortgaged_of,owned_tiles,player_out,set_mortgaged,set_owner,target_tile,tile_count,tile_group,tile_owner,transfer` | yes | — |
| `mor` | `Mor:（小白）` | `effect::has,log,pay` | yes | — |
| `mor` | `Mor:（筑紫）迷茫的庭园` | `ask_yes,cant_move,card_move,gain,log,plan::set_kind,plan::set_resolve,plan::set_steps,plan::set_teleport_to,player_count,player_out,roll,state::TURN_START,state::add,state::set_expires,tile_count,tile_steps_ahead` | yes | — |
| `mujica` | `Mujica:#J11` | `ask_yes,crystals,decay,dist,give_stun,is_placed,log,others,place_card,player_pos,set_crystals,set_dest` | yes | — |
| `mujica` | `Mujica:人偶的箱庭` | `ask_pick,cant_move,card_move,log,others,plan::set_resolve,plan::set_steps,player_pos,roll,stay_of,stun_of,tile_forward,transfer` | yes | — |
| `mujica` | `Mujica:会被骗着买水晶的人` | `add_counter_at,ask_pick,character_is,counter_at,field_instances,is_face_down_at,is_immune_at,log,player_count,player_out` | yes | — |
| `mujica` | `Mujica:心の雨` | `gain,give_stay,give_stun,log,others,player_pos,roll,tile_forward` | yes | — |
| `mujica` | `Mujica:无法将视线移开` | `ask_number,ask_pick,card_move,log,others,plan::set_resolve,plan::set_reverse,plan::set_steps` | yes | — |
| `mujica` | `Mujica:欢迎来到ave mujica的世界` | `ask_player,log,others,slot,state::get,state::set,target,turn_key` | yes | — |
| `mujica` | `Mujica:燃尽前的线香花火` | `add_crystals,give_extra_turn,give_stun,log,place_card,set_dest` | yes | — |
| `mujica` | `Mujica:祥，移动` | `ask_pick,ask_player,cant_move,card_move,log,others,plan::set_pay_factor,plan::set_resolve,plan::set_reverse,plan::set_steps,target` | yes | — |
| `mujica` | `Mujica:骰子已经掷下` | `is_placed,log,place_card,set_dest,set_slot,slot,turn_key` | yes | — |
| `mujica` | `Mujica:黑色生日` | `money_of,others,player_out,set_slot,slot,transfer` | yes | — |
| `mujica` | `Mujica:（初华）我，无畏悲伤` | `ask_number,ask_pick,cant_move,draw,fixed_roll,log,set_fixed_roll,state::get` | yes | — |
| `mujica` | `Mujica:（喵梦）` | `draw,log,place_card,self_face_down,set_dest,set_slot,slot,trigger::player_id` | yes | — |
| `mujica` | `Mujica:（海铃）` | `add_band_skill,band_skills,draw,log,others,place_card,player_count,player_out,set_dest,set_slot,slot,unplace_at,unplace_self` | yes | — |
| `mujica` | `Mujica:（睦/mortis）表演的本能` | `log` | yes | — |
| `mujica` | `Mujica:（祥子）斩断留恋，忘却一切` | `ask_card,ask_tile,cant_move,card_mortgage,card_move,cards_in,discard_from_hand,hand_size,is_buyable,is_ring,log,mortgaged_of,owned_tiles,plan::set_kind,plan::set_resolve,plan::set_teleport_to,tile_count,tile_owner,tile_price` | yes | — |
| `mygo` | `MyGO:[千早爱音]Anon Tokyo` | `ask_tile,cant_move,card_move,count_marks,is_buyable,is_placed,log,pay,place_card,place_mark,plan::set_reverse,plan::set_steps,player_pos,rent_of,self_uid,set_dest,set_slot,slot,target_tile,tile_count,tile_forward,tile_owner,tile_price` | yes | — |
| `mygo` | `MyGO:即使迷茫着` | `card_move,effect::hits,hand_size,is_placed,log,place_card,plan::set_resolve,plan::set_steps,send_to_dest,set_dest` | yes | — |
| `mygo` | `MyGO:哪怕这旅程没有终点` | `decay,gain,log,place_card_on,player_pos,set_crystals,set_dest,turn_main_steps` | yes | — |
| `mygo` | `MyGO:壱雫空` | `gain,give_stay,give_stun,log,player_count,player_out,slot,state::get,state::set,stay_of,stun_of,transfer` | yes | — |
| `mygo` | `MyGO:无路矢` | `ask_player,give_exile,is_placed,log,others,place_card,player_pos,set_dest,set_slot,slot,state::get,target,trigger::pay_is_rent,trigger::set_pay_target,trigger::target` | yes | — |
| `mygo` | `MyGO:普通与理所当然` | `effect::has,log,set_next_steps,slot` | yes | — |
| `mygo` | `MyGO:羽丘的不可思议女孩` | `add_house,ask_yes,draw,is_placed,log,mortgaged_of,place_card,player_pos,roll,set_dest,tile_owner` | yes | — |
| `mygo` | `MyGO:若能再次交汇` | `do_move_roll,log,others,player_pos,tile_count,tile_forward` | yes | — |
| `mygo` | `MyGO:轮符雨` | `before_turn_end,card_settle_at,give_stay,log,player_pos` | yes | — |
| `mygo` | `MyGO:那天的雨` | `crystals,decay,give_stay,is_placed,log,place_card,player_count,player_out,player_pos,roll,set_crystals,set_dest,tile_count,tile_named` | yes | — |
| `mygo` | `MyGO:难以复刻的奇迹` | `add_band_crystals,add_crystals,band_crystals,build_cost,counter_at,crystals,field_instances,gain,log,money_of,others,place_card,players_on,set_build_discount,set_crystals,set_dest,set_slot,slot` | yes | — |
| `mygo` | `MyGO:（soyo）混合的颜色` | `ask_tile,is_placed,log,owned_count,owned_tiles,place_card_on,self_tile,set_dest,set_tile_prop,trigger::pay_is_rent,trigger::set_pay_amount,trigger::target,trigger::tile,trigger::value` | yes | — |
| `mygo` | `MyGO:（乐奈）有趣的女人` | `add_crystals,add_tok,ask_yes,crystals,gate,log,place_card_on,plan::set_pay_factor,plan::set_resolve,plan::set_stop_at,player_pos,self_tile,set_dest,set_transfer_to_dest,tok` | yes | — |
| `mygo` | `MyGO:（灯）不再迷茫` | `add_crystals,ask_yes,cards_in,discard_from_hand,gain_fire,give_stay,inc_slot,log,place_card,set_crystals,set_dest,set_slot,slot,trigger::player_id,trigger::value` | yes | — |
| `mygo` | `MyGO:（立希）想认真去做` | `ask_tile,card_settle_at,log,plan::set_pay_factor,player_count,player_out,player_pos,stay_of,tile_count` | yes | — |
| `pp` | `PP:TITLE IDOL` | `add_band_crystals,add_counter_at,card_text_mentions,field_instances,is_face_down_at,is_immune_at,log` | yes | — |
| `pp` | `PP:[丸山彩]憧憬的前方` | `add_to_hand,ask_card,cards_in,log,money_of,place_card,player_count,player_out,set_dest,take_card,tok` | yes | — |
| `pp` | `PP:[冰川日菜]会发出怎样的声音呢？` | `ask_card,log,play_card,set_dest` | yes | — |
| `pp` | `PP:[大和麻弥]可能性为∞` | `add_band_crystals,add_crystals,add_to_deck,cards_in,crystals,is_placed,log,to_discard` | yes | — |
| `pp` | `PP:[白鹭千圣]微笑的铁假面` | `designation_cancelled,is_placed,others,pay_leg,pay_total,place_card,player_out,set_dest` | yes | — |
| `pp` | `PP:[若宫伊芙]属于我的武士道！` | `add_crystals,crystals,gain,houses_of,is_placed,log,place_card,player_out,roll,set_dest,tile_owner,transfer` | yes | — |
| `pp` | `PP:[衍生]共鸣` | `add_band_crystals,ask_yes,hand_count,in_band,take_from_hand,to_discard` | yes | — |
| `pp` | `PP:[衍生]明天见` | `add_tok,hand_count,log,tok` | yes | — |
| `pp` | `PP:[衍生]重叠的声音` | `add_to_hand,add_tok,at_turn_end,cant_move,card_move,find_card,give_stay,log,plan::set_resolve,plan::set_teleport_to,set_dest,tile_named,unplace_at` | yes | — |
| `pp` | `PP:[衍生]魔法战队Pastel✽Ranger` | `add_band_crystals,cards_in,dist,draw,gain,houses_of,log,owned_tiles,pay,player_pos,set_build_discount,tok` | yes | — |
| `pp` | `PP:不要背负期待` | `add_to_deck,draw,inc_slot,log,place_card,plan::set_resolve,set_dest,set_self_immune,set_slot,slot` | yes | — |
| `pp` | `PP:再次闪耀` | `add_tok,ask_pick,gain,is_buyable,is_placed,log,money_of,owned_tiles,place_card,set_dest,set_self_immune,set_slot,slot,tile_group,tile_price` | yes | — |
| `pp` | `PP:初次演出事故` | `add_to_deck,draw,log,place_card,set_dest,set_tok` | yes | — |
| `pp` | `PP:可爱又强壮的花朵` | `add_band_crystals,ask_tile,gain,log,owned_count,owned_tiles,place_card_on,self_tile,set_dest` | yes | — |
| `pp` | `PP:同一个梦想` | `add_band_crystals,add_tok,gain,in_band,log,player_count,tok` | yes | — |
| `pp` | `PP:找回珍妮弗` | `add_band_crystals,add_to_hand,add_tok,ask_player,in_band,log,others,others_count,place_card,player_out,send_to_dest,set_dest,set_slot,slot,target,tok,transfer` | yes | — |
| `pp` | `PP:有你与我在这里共度` | `add_tok,draw,log,tok` | yes | — |
| `pp` | `PP:梦在前方，结彩当下` | `add_crystals,ask_pick,card_buy,crystals,dist,inc_slot,inc_start_hand,is_buyable,is_placed,log,place_card,player_out,player_pos,set_dest,slot,take_card,tile_count,tile_owner,tok,transfer` | yes | — |
| `pp` | `PP:练习生解密指南` | `add_band_crystals,add_crystals,add_tok,cards_in,crystals,inc_slot,is_placed,log,pay,place_card,set_build_discount,set_crystals,set_dest,slot,state::add,state::get,tok,trigger::player_id` | yes | — |
| `ppp` | `PPP:Bang Dream!` | `add_band_crystals,ask_tile,card_offer_build,log,owned_count,owned_tiles,teleport_to` | yes | — |
| `ppp` | `PPP:Popipa` | `add_to_deck,draw,gain,log,set_dest` | yes | — |
| `ppp` | `PPP:Returns` | `add_to_deck,add_tok,ask_card,ask_player_cards,ask_yes,band_skill,cards_in,character_is,in_band,is_placed,log,others,place_card,player_out,prop,roll,set_dest,set_prop,set_tok,take_card,trigger::cards,trigger::player_id,unplace_card_named` | yes | — |
| `ppp` | `PPP:STAR BEAT!` | `add_tok,ask_pick,ask_yes,can_build_on,cant_move,card_move,card_offer_build,dist,inc_slot,log,money_of,owned_tiles,plan::set_base_dice,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_count,player_out,player_pos,slot,tile_count,trigger::player_id` | yes | — |
| `ppp` | `PPP:Tomorrow's Door` | `houses_of,log,mortgaged_of,place_card_on,player_out,self_tile,set_dest,set_self_tile,set_slot,slot,tile_named,tile_owner,transfer` | yes | — |
| `ppp` | `PPP:[衍生]Pipopa` | `gain,place_card,set_dest` | yes | — |
| `ppp` | `PPP:[衍生]Popipapapipopa` | `add_crystals,ask_number,crystals,log` | yes | — |
| `ppp` | `PPP:仓库里的Random Star` | `add_to_deck_at,add_tok,ask_yes,log,place_card,plan::set_resolve,plan::set_stop_at,set_dest,sweep_to_deck,tile_named` | yes | — |
| `ppp` | `PPP:向着未来的路标` | `before_turn_end,cant_move,card_move,log,pay,plan::set_resolve,plan::set_steps` | yes | — |
| `ppp` | `PPP:抓到了` | `ask_player,cant_move,card_move,card_offer_build,log,others,plan::set_steps,player_pos,tile_forward` | yes | — |
| `ppp` | `PPP:献给远方的你` | `ask_player,card_move,card_offer_build,dist,log,others,owned_tiles,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_out,player_pos` | yes | — |
| `ppp` | `PPP:迷宫般的仓库` | `cant_move,card_move,dist,is_buyable,linger,log,pay,plan::set_start,plan::set_steps,plan::set_stop_at,player_out,player_pos,roll,tile_count,tile_named,tile_owner` | yes | — |
| `ppp` | `PPP:（多惠）寻找更好的声音` | `gain_fire,teleport_to,tile_named` | yes | — |
| `ppp` | `PPP:（有咲）等等等一下` | `add_crystals,crystals,is_placed,log,place_card,set_dest` | yes | — |
| `ppp` | `PPP:（沙绫）总有一天要给这片天空命名` | `gain_fire,log,place_card_on,roll,self_tile,set_dest,set_self_tile,set_slot,tile_count,tile_named` | yes | — |
| `ppp` | `PPP:（里美）我的心就像巧克力螺` | `ask_tile,cant_move,card_move,log,plan::set_reverse,plan::set_steps,player_pos,state::TURN_END,state::set,state::set_expires,tile_count,tile_forward` | yes | — |
| `ppp` | `PPP:（香澄）大家我都喜欢哦` | `add_band_crystals,gate,log,place_card_on,plan::set_rent_factor,plan::set_resolve,plan::set_stop_at,player_out,self_tile,set_dest,tile_named` | yes | — |
| `r` | `R:Fire bird` | `ask_number,crystals,decay,is_placed,log,pay,place_card,set_crystals,set_dest` | yes | — |
| `r` | `R:NFO` | `add_to_deck_at,add_to_hand,add_tok,ask_card,ask_yes,at_turn_end,card_move,card_replayable,cards_in,effect,gain,give_stay,log,others,place_card,plan::set_resolve,plan::set_steps,play_card,player_out,player_pos,players_on,roll,set_dest,split_pay,take_card,tile_count,tile_forward,tile_steps_ahead,to_discard,tok,trigger::set_pay_amount,trigger::target,trigger::value` | yes | — |
| `r` | `R:Sprechchor` | `gain,is_live_house_for,roll,turn_start_pos` | yes | — |
| `r` | `R:[衍生] 压` | `gain` | yes | — |
| `r` | `R:[衍生] 觉悟` | `gain,set_dest` | yes | — |
| `r` | `R:live前的准备` | `gate,log,plan::set_resolve,plan::set_stop_at,tile_named` | yes | — |
| `r` | `R:向着顶点` | `add_house,ask_tile,ask_yes,build_cost,can_pay,card_move,houses_of,is_buyable,is_color,log,money_of,mortgaged_of,pay,plan::set_resolve,plan::set_steps,player_pos,rent_of,state::get,stay_of,stun_of,tile_count,tile_forward,tile_owner` | yes | — |
| `r` | `R:学生会的检查` | `add_to_deck,ask_tile,ask_yes,build_cost,dist,gain,gate,linger,log,owned_tiles,pay,place_card_on,plan::set_rent_factor,plan::set_resolve,plan::set_stop_at,player_pos,rent_of,self_tile,set_dest,set_prop,set_slot,slot` | yes | — |
| `r` | `R:必然的联系（莉莎）` | `ask_player,log,place_card_at,player_pos,players_on,set_dest,target` | yes | — |
| `r` | `R:曲奇时间` | `discard_size,gain,log,shuffle_into_deck` | yes | — |
| `r` | `R:蓝玫瑰的骄傲` | `cant_move,card_move,gain,is_buyable,is_color,log,owned_count,owned_tiles,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_pos,tile_count,tile_forward,tile_named,tile_owner,tile_price` | yes | — |
| `r` | `R:轨迹` | `ask_tile,log,owned_count,owned_tiles,set_houses,set_mortgaged,set_owner` | yes | — |
| `r` | `R:选择自己的舞台` | `ask_yes,effect::count,effect::kind,effect::value,log,set_slot` | yes | — |
| `r` | `R:（ykn）louder` | `add_ring_bonus,log` | yes | — |
| `r` | `R:（亚子）黑暗大魔姬亚子` | `draw,effect::has,give_stun,log,play_from_hand,stun_of` | yes | — |
| `r` | `R:（燐子）Ringing Bloom` | `gain,houses_of,is_ring,log,owned_tiles,place_card,rent_houses_of,set_dest,set_prop,set_tile_prop` | yes | — |
| `r` | `R:（纱夜）弹奏弹奏弹奏，继续弹奏` | `character_skill,log,skill_blocked,turn_key` | yes | — |
| `ras` | `RAS:Change the world` | `add_crystals,ask_tile,cant_move,crystals,houses_of,log,place_card_on,plan::set_base_dice,self_tile,set_dest,set_slot,tile_named,tile_owner,turn_key` | yes | — |
| `ras` | `RAS:EXIST` | `draw,is_placed,log,place_card,prop,set_dest,set_prop` | yes | — |
| `ras` | `RAS:PLEASE CHOOSE` | `ask_pick,is_live_house_for,log,player_pos,teleport_to` | yes | — |
| `ras` | `RAS:R. I. O. T.` | `cards_in,discard_from_hand,draw,effect::hits,player_count,player_out` | yes | — |
| `ras` | `RAS:Repaint` | `log,plan::set_pay_factor,target,tile_owner,tile_steps_ahead` | yes | — |
| `ras` | `RAS:UNSTOPPABLE` | `cant_move,card_move,gain,plan::set_resolve,plan::set_teleport_to,player_out,roll,tile_named` | yes | — |
| `ras` | `RAS:成为最强` | `cant_move,card_move,is_live_house_for,log,owned_tiles,plan::set_teleport_to,roll,tile_named` | yes | — |
| `ras` | `RAS:游击演出` | `ask_tile,ask_yes,before_turn_end,buy_price,card_buy,card_settle_at,discard_count,is_buyable,log,money_of,owned_tiles,paid_in_settle,place_card,set_dest,set_slot,set_tile_prop,slot,take_card,teleport_to,tile_count,tile_named,tile_owner,tile_price,turn_player` | yes | — |
| `ras` | `RAS:狂乱Hey Kids!!` | `add_house,ask_number,ask_tile,build_cost,can_build_on,gain,houses_of,log,owned_tiles,pay` | yes | — |
| `ras` | `RAS:练习室里的风暴` | `add_crystals,card_settle_at,crystals,dist,log,place_card_on,plan::set_rent_factor,player_pos,self_tile,send_to_dest,set_dest,tile_named,tile_owner` | yes | — |
| `ras` | `RAS:（LOCK）追逐梦想的步伐` | `cards_in,give_exile,log,place_card,send_to_dest,set_dest,set_slot,take_card,teleport_to,tile_named` | yes | — |
| `ras` | `RAS:（MASKING）CRUSH ON THE DRUM!!!` | `cant_move,discard_size,log,plan::add_extra_dice` | yes | — |
| `ras` | `RAS:（PAREO）渐渐远去的你` | `add_house,add_tok,ask_tile,ask_yes,character_skill,houses_of,invoke_skill,log,owned_tiles,tok` | yes | — |
| `ras` | `RAS:（chuchu）演奏我的音乐吧` | `add_crystals,ask_player,draw,gain,is_placed,log,others,place_card,player_count,player_out,set_crystals,set_dest,set_slot,set_transfer_to_dest,slot,target,unplace_self` | yes | — |
| `ras` | `RAS:（和奏瑞依）寄于指尖的执念` | `ask_pick,fire,is_placed,log,place_card,set_dest,set_fixed_roll,set_slot,slot,spend_fire` | yes | — |
| `sumimi` | `Sumimi:#L11` | `log,place_card,set_crystals,set_dest,trigger::player_id` | yes | — |
| `sumimi` | `Sumimi:#L12` | `add_crystals,add_to_hand,ask_card,cards_in,crystals,is_placed,log,place_card,set_dest,take_card` | yes | — |
| `sumimi` | `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励` | `at_turn_end,cant_move,gain_fire,log,place_card,plan::set_start,set_dest,tile_named` | yes | — |
| `sumimi` | `Sumimi:Here the world` | `add_crystals,add_to_hand,draw,log,place_card,player_out,set_crystals,set_dest,set_slot,slot,take_from_hand` | yes | — |
| `sumimi` | `Sumimi:Sumimi不会解散哦` | `ask_tile,cant_move,card_move,log,money_of,mortgaged_of,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_out,player_pos,rent_of,state::get,tile_count,tile_owner` | yes | — |
| `sumimi` | `Sumimi:Sumimi是二人一体的` | `character_is,find_card,log,place_card,state::set,state::set_bounds,unplace_at` | yes | — |
| `sumimi` | `Sumimi:Sweet Escape` | `ask_tile,buy_price,cant_move,card_buy,card_move,houses_of,is_buyable,log,money_of,mortgaged_of,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_pos,rent_of,ring_multiplier,tile_count,tile_group,tile_named,tile_owner,tile_price` | yes | — |
| `sumimi` | `Sumimi:一人两个甜甜圈` | `ask_tile,ask_yes,card_move,gain,gain_fire,give_exile,log,place_card,plan::set_kind,plan::set_resolve,plan::set_teleport_to,player_out,player_pos,set_dest,set_slot,slot,teleport_to,tile_count` | yes | — |
| `sumimi` | `Sumimi:兼顾偶像与乐队` | `gain,log,money_of,state::get` | yes | — |
| `sumimi` | `Sumimi:现在她是Sumimi的小初啦` | `gain,houses_of,is_buyable,player_pos,rent_of,ring_multiplier,tile_price` | yes | — |
| `sumimi` | `Sumimi:（真奈）歌唱大赛5连冠` | `draw,effect::hits,log` | yes | — |
| `events` | `event:A！A！O！` | `at_next_turn_end,log,set_prop` | yes | — |
| `events` | `event:EX任务挑战` | `log` | yes | — |
| `events` | `event:Forbidden Moca` | `at_next_turn_end,log` | yes | — |
| `events` | `event:Kizuna Music` | `cards_in,give_exile,log,teleport_to` | yes | — |
| `events` | `event:PICO灵魂交换` | `log,player_pos,set_slot,slot,state::get,teleport_to` | yes | — |
| `events` | `event:一切不会结束` | `give_exile,log,teleport_to,tile_named` | yes | — |
| `events` | `event:上学时间` | `character_is,give_exile,log,teleport_to,tile_named` | yes | — |
| `events` | `event:修复公告` | `build_cost,log,prop,set_build_discount,set_prop,tile_count,tile_prop` | yes | — |
| `events` | `event:元祖！邦多利酱` | `log,prop,set_prop` | yes | — |
| `events` | `event:冲榜` | `ask_yes,draw,fire,gain,log,spend_fire` | yes | — |
| `events` | `event:前场队还是后场队？` | `gain,log,pay_leg,pay_total,player_pos` | yes | — |
| `events` | `event:前往哈比内尔王国旅游` | `give_exile,log,tile_named` | yes | — |
| `events` | `event:协助CiRCLE重建` | `ask_pick,decay,draw,give_stay,is_circle,log,pay,set_crystals,set_tile_prop,tile_named` | yes | — |
| `events` | `event:卡池BUG` | `build_cost,event_deck_push,log,set_prop` | yes | — |
| `events` | `event:发送熊饼表情` | `ask_number,band_skill,card_text_mentions,character_skill,event_deck_push,fire,fire_max,gain_fire,log,money_of,pay` | yes | — |
| `events` | `event:对邦` | `log,transfer` | yes | — |
| `events` | `event:幻觉来了` | `log,plan::set_roller,player_count,player_out,prop,set_prop` | yes | — |
| `events` | `event:弦卷集团地产开发` | `ask_pick,is_buyable,log,money_of,pay,raise_bought,tile_count,tile_owner` | yes | — |
| `events` | `event:很噜的感觉` | `ask_yes,log,money_of,pay,teleport_to,tile_named` | yes | — |
| `events` | `event:意外的对邦` | `log,player_pos,prop,set_prop,tile_count,tile_forward,turn_player` | yes | — |
| `events` | `event:泪水的含义` | `owned_tiles,tile_group,transfer` | yes | — |
| `events` | `event:火种燃尽之后会怎么样呢？` | `count_marks,log,pay,place_mark,player_pos,prop,remove_marks,self_uid,set_prop,state::get,tile_count,tile_steps_ahead` | yes | — |
| `events` | `event:让我来结束一切` | `ask_pick,event_banish,log` | yes | — |
| `events` | `event:超燃甩头` | `at_next_turn_end,at_turn_end,crystals,log,set_crystals` | yes | — |
| `events` | `event:这只手我不会放开` | `card_move,log,plan::set_resolve,plan::set_reverse,plan::set_steps` | yes | — |
| `events` | `event:迷子的追逐` | `event_deck_push,event_is_active,log` | yes | — |
| `events` | `event:飞鸟山之战` | `card_move,give_exile,log,neighbor,state::TURN_START,state::add,state::set,state::set_expires,teleport_to,tile_named` | yes | — |
| `events` | `event:麻里奈小姐的礼物箱` | `ask_pick,gain,log,money_of,pay,prop,set_prop` | yes | — |
| `tile_marks` | `mark:cp` | `add_counter_at,bump_mark,count_marks,counter_at,gain,mark_src_at,message::a,message::reply,message::sender_uid,message::tile,place_mark,tile_count` | yes | — |
| `skills_bands` | `skill:Afterglow:商店街的宠儿` | `build_cost,card_build,is_shop,log,set_build_discount,tile_price,trigger::player_id,trigger::tile` | yes | — |
| `skills_bands` | `skill:Ave Mujica:假面之下的真实` | `add_counter_at,add_crystals,counter_at,crystals,find_card,is_placed,log,skill_blocked,spend_fire,trigger::player_id,trigger::set_pay_amount,trigger::target,trigger::value` | yes | — |
| `skills_bands` | `skill:CRYCHIC:美好的往日幻影` | `add_crystals,add_to_deck,ask_yes,cards_in,crystals,deck_count,discard_from_hand,discard_size,draw,field_instances,gain,hand_size,log,place_card,shuffle_into_deck,sweep_to_deck,take_card,take_from_hand,trigger::player_id,trigger::set_cancelled,trigger::set_pay_amount,trigger::target,trigger::value,unplace_at` | yes | — |
| `skills_bands` | `skill:CiRCLE:后勤人员的努力` | `ask_number,ask_yes,cards_in,discard_from_hand,hand_size,log,set_play_doubled,trigger::cards,trigger::player_id` | yes | — |
| `skills_bands` | `skill:Hello, Happy World!:传播笑容` | `add_crystals,ask_yes,crystals,log,set_build_discount,trigger::pay_is_rent,trigger::player_id,trigger::set_pay_amount,trigger::target,trigger::value` | yes | — |
| `skills_bands` | `skill:Morfonica:振翅高飞的练习曲` | `draw,gain,log,trigger::player_id,trigger::set_cancelled,trigger::target,trigger::value` | yes | — |
| `skills_bands` | `skill:MyGO!!!!!:迷途之星` | `add_crystals,ask_pick,card_move,crystals,draw,log,roll,skill_blocked,teleport_to,tile_count,trigger::player_id,turn_player` | yes | — |
| `skills_bands` | `skill:Pastel✽Palettes:与偶像一起` | `add_crystals,add_tok,ask_number,ask_yes,crystals,draw,in_band,log,placed_cards,player_count,player_out,set_prop,skill_blocked,tok,trigger::player_id` | yes | — |
| `skills_bands` | `skill:Poppin' Party:星之鼓动` | `add_counter_at,add_crystals,add_tok,ask_pick,ask_yes,counter_at,crystals,draw,find_card,gain,in_band,is_circle,log,mortgaged_of,others,pay_leg,pay_total,player_count,player_out,set_prop,skill_blocked,tile_named,tok,trigger::move_is_main,trigger::player_id,trigger::set_cancelled,trigger::set_pay_amount,trigger::set_reason,trigger::tile,trigger::value` | yes | — |
| `skills_bands` | `skill:RAISE A SUILEN:UNSTOPPABLE` | `ask_yes,is_live_house_for,log,owned_tiles,player_pos,trigger::by_card,trigger::player_id,trigger::set_cancelled,trigger::tile` | yes | — |
| `skills_bands` | `skill:Roselia:对音乐的纯粹` | `is_live_house_for,log,owned_tiles,set_tile_prop,tile_group,trigger::player_id,trigger::price,trigger::set_price,trigger::tile` | yes | — |
| `skills_bands` | `skill:Sumimi:人气偶像组合` | `add_crystals,crystals,log,trigger::player_id,trigger::set_pay_amount,trigger::target,trigger::value` | yes | — |
| `skills_characters` | `skill:三角初华:Imprisoned XII` | `ask_yes,log,plan::set_extra_steps,players_on,roll,tile_named,transfer,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:三角初华（Sumimi）:成为偶像` | `ask_pick,ask_yes,bump_mark,count_marks,gain_fire,is_color,log,place_mark,self_uid,spend_fire,tile_count,tile_group,tile_named,transfer,trigger::move_remaining,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:上原绯玛丽:大家一起迈出新的一步` | `ask_pick,gain_fire,in_band,log,skill_blocked,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:丰川祥子:请把你们的人生交给我` | `ask_yes,cards_in,discard_from_hand,gain,gain_fire,give_stay,give_stun,log,play_card,players_on,stay_of,stun_of,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？` | `ask_yes,dist,gain_fire,log,plan::set_kind,plan::set_teleport_to,player_pos,spend_fire,tile_count,transfer,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:丸山彩:With~` | `add_tok,ask_number,in_band,log,place_card,player_count,player_out,skill_blocked,tok,trigger::player_id,trigger::set_pay_amount,trigger::value` | yes | — |
| `skills_characters` | `skill:二叶筑紫:交给班长吧` | `add_tok,ask_pick,ask_yes,gain,log,player_count,player_out,skill_blocked,slot,tok,trigger::set_pay_amount,trigger::target,trigger::value` | yes | — |
| `skills_characters` | `skill:今井莉莎:慈爱女神` | `ask_yes,gain_fire,log,player_count,player_out,player_pos,skill_blocked,spend_fire,tile_count,tile_forward,trigger::player_id` | yes | — |
| `skills_characters` | `skill:仓田真白:向后全速前进` | `log` | yes | — |
| `skills_characters` | `skill:佐藤益木:与燃烧的红色一起驰骋` | `ask_number,gain_fire,log,skill_blocked,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:八幡海铃:熟练的支援贝斯手` | `ask_number,ask_player,ask_tile,ask_yes,card_settle_at,draw,gain,gain_fire,houses_of,log,money_of,others,owned_tiles,players_on,skill_blocked,spend_fire,tile_count,tile_owner,transfer,trigger::player_id,trigger::set_pay_amount,trigger::tile,trigger::value,turn_key` | yes | — |
| `skills_characters` | `skill:八潮瑠唯:正论暴击` | `effect,log,roll,trigger::player_id,trigger::set_pay_amount,trigger::target,trigger::value` | yes | — |
| `skills_characters` | `skill:冰川日菜:日菜抽中的大奖` | `add_tok,find_card,in_band,log,place_card,player_count,player_out,roll,trigger::player_id,unplace_at` | yes | — |
| `skills_characters` | `skill:冰川纱夜:踏上荆棘之路的觉悟` | `gain_fire,log,player_count,player_out,skill_blocked,spend_fire,trigger::player_id,turn_key` | yes | — |
| `skills_characters` | `skill:凑友希那:来练习吧` | `ask_yes,gain_fire,gate,is_circle,is_ring,log,plan::set_stop_at,player_pos,set_owner,set_prop,spend_fire,teleport_to,tile_named,trigger::move_remaining,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:北泽育美:全垒打！` | `add_tok,ask_yes,bump_mark,count_marks,log,place_mark,self_uid,tile_named,tok,transfer,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:千早爱音:重新开始` | `ask_yes,draw,gain_fire,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:和奏瑞依:一次又一次竭尽全力` | `ask_pick,ask_yes,do_move_roll_ask,fixed_roll,gain_fire,log,spend_fire,trigger::move_roll,trigger::player_id,trigger::set_move_roll,trigger::value` | yes | — |
| `skills_characters` | `skill:大和麻弥:朝阳照耀的片刻` | `add_to_deck,add_to_hand,add_tok,ask_number,ask_pick,cards_in,in_band,log,place_card,player_count,player_out,take_card,tok,trigger::player_id` | yes | — |
| `skills_characters` | `skill:奥泽美咲:另一个我` | `ask_yes,gain_fire,give_exile,log,skill_blocked,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:宇田川亚子:对帅气的憧憬` | `add_to_hand,ask_pick,ask_yes,cards_in,field_instances,is_face_down_at,log,pay,place_card,set_dest,set_face_down_at,take_card,to_discard,trigger::player_id,unplace_at` | yes | — |
| `skills_characters` | `skill:宇田川巴:豚骨酱油拉面大姐` | `dist,gain_fire,in_band,log,player_pos,skill_blocked,spend_fire,tile_named,tile_steps_ahead,trigger::player_id` | yes | — |
| `skills_characters` | `skill:山吹沙绫:焕然一新的天空中` | `add_tok,ask_yes,gain,gain_fire,is_buyable,log,plan::set_kind,plan::set_teleport_to,player_out,player_pos,roll,spend_fire,tile_count,tok,trigger::move_roll,trigger::player_id,trigger::set_move_roll,trigger::set_pay_amount,trigger::value` | yes | — |
| `skills_characters` | `skill:市谷有咲:盆栽爱好者` | `add_band_crystals,ask_yes,draw,gain_fire,log,player_pos,set_slot,slot,spend_fire,tile_named,trigger::move_is_main,trigger::player_id` | yes | — |
| `skills_characters` | `skill:广町七深:这是很普通的事吧？` | `add_tok,ask_pick,ask_yes,gain_fire,log,play_card,player_count,player_out,skill_blocked,spend_fire,tok,tok_names,trigger::cards,trigger::player_id` | yes | — |
| `skills_characters` | `skill:弦卷心:弦卷集团` | `gain` | yes | — |
| `skills_characters` | `skill:户山香澄:非凡之星` | `ask_pick,card_move,gain_fire,owned_tiles,skill_blocked,spend_fire,tile_named,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:月岛麻里奈:礼物还有好多好多哟` | `ask_yes,gain,roll,transfer,trigger::player_id` | yes | — |
| `skills_characters` | `skill:朝日六花:瞄准目标` | `is_buyable,is_live_house_for,log,owned_tiles,set_tile_prop,tile_group,tile_named,tile_owner,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:松原花音:真正的迷子` | `do_move_roll,log,teleport_to,tile_named,trigger::move_roll,trigger::player_id,trigger::set_move_roll,trigger::value` | yes | — |
| `skills_characters` | `skill:桐谷透子:天上天下，唯我独尊` | `ask_pick,gain_fire,log,owned_tiles,player_out,players_on,rent_of,skill_blocked,spend_fire,tile_count,transfer,trigger::player_id` | yes | — |
| `skills_characters` | `skill:椎名立希:决定练习日的会议` | `add_counter_at,ask_pick,field_instances,gain_fire,log,skill_blocked,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:椎名立希（CRYCHIC）:克服劣等感` | `ask_yes,do_move_roll,log,trigger::move_roll,trigger::player_id,trigger::set_move_roll` | yes | — |
| `skills_characters` | `skill:濑田薰:梦幻的王子殿下` | `ask_pick,bump_mark,count_marks,dist,gain_fire,log,owned_tiles,place_mark,plan::set_start,player_count,player_out,player_pos,self_uid,skill_blocked,spend_fire,tile_count,tile_named` | yes | — |
| `skills_characters` | `skill:牛込里美:里美的决心` | `ask_pick,card_move,card_offer_build,dist,gain_fire,is_buyable,is_live_house_for,player_pos,skill_blocked,spend_fire,tile_count,trigger::player_id,trigger::tile,turn_player` | yes | — |
| `skills_characters` | `skill:珠手知由:天才制作人` | `log,set_build_cost_pct,trigger::player_id` | yes | — |
| `skills_characters` | `skill:白金燐子:即使1cm也要前进` | `ask_number,gain_fire,log,set_fixed_roll,skill_blocked,spend_fire,state::TURN_END,trigger::player_id` | yes | — |
| `skills_characters` | `skill:白鹭千圣:保持坦率的你` | `add_tok,ask_number,in_band,log,place_card,player_count,player_out,skill_blocked,tok,trigger::set_pay_amount,trigger::value` | yes | — |
| `skills_characters` | `skill:祐天寺若麦:大主播喵梦亲` | `field_instances,gain_fire,is_face_down_at,is_immune_at,log,set_face_down_at,set_immune_at,trigger::player_id,trigger::set_pay_amount,trigger::value` | yes | — |
| `skills_characters` | `skill:纯田真奈:甜甜圈爱好者` | `ask_yes,buy_price,gain_fire,log,pay,rent_of,spend_fire,state::TURN_END,tile_named,tile_owner,transfer,trigger::set_pay_amount,trigger::tile,trigger::value` | yes | — |
| `skills_characters` | `skill:美竹兰:叛逆的红挑染` | `gain_fire,in_band,log,set_prop,skill_blocked,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:羽泽鸫:伟大的平凡` | `at_turn_end,gain_fire,in_band,log,set_prop,skill_blocked,spend_fire,trigger::player_id` | yes | — |
| `skills_characters` | `skill:花园多惠:花园警察，出警！` | `ask_yes,count_marks,gain,gain_fire,is_placed,log,place_mark,player_out,remove_marks,roll,self_uid,spend_fire,tile_count,tile_named,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:若叶睦:天生的演员` | `ask_yes,draw,hand_size,log,trigger::player_id` | yes | — |
| `skills_characters` | `skill:若叶睦（CRYCHIC）:精致的人偶` | `ask_pick,gain_fire,log,player_count,player_out,set_fixed_roll,skill_blocked,spend_fire,trigger::move_roll,trigger::player_id,trigger::value` | yes | — |
| `skills_characters` | `skill:若宫伊芙:天下统一` | `add_tok,ask_number,fixed_roll,in_band,log,place_card,player_count,player_out,roll,skill_blocked,tok,trigger::move_roll,trigger::set_move_roll,trigger::value` | yes | — |
| `skills_characters` | `skill:要乐奈:投币式停车场的猫` | `add_tok,ask_yes,card_move,draw,gain,gain_fire,is_circle,is_ring,log,move_units,place_mark,self_uid,skill_blocked,spend_fire,tile_named,tile_owner,tok,transfer,trigger::player_id,trigger::target,trigger::tile` | yes | — |
| `skills_characters` | `skill:都筑诗船:尽力了吗` | `ask_pick,build_cost,card_settle_at,gain,gain_fire,log,player_count,player_out,player_pos,roll,set_owner,skill_blocked,slot,spend_fire,state::TURN_END,tile_named,tile_owner,trigger::move_roll,trigger::player_id,trigger::set_move_roll,trigger::set_pay_amount,trigger::tile,trigger::value` | yes | — |
| `skills_characters` | `skill:长崎素世:通透的颜色` | `ask_yes,gain_fire,log,set_tile_prop,spend_fire,tile_group,trigger::player_id,trigger::tile` | yes | — |
| `skills_characters` | `skill:长崎素世（CRYCHIC）:雨中祈晴` | `ask_yes,log,trigger::player_id,trigger::set_pay_amount,trigger::tile` | yes | — |
| `skills_characters` | `skill:青叶摩卡:我行我素` | `ask_yes,gain_fire,in_band,log,player_pos,spend_fire,teleport_to,trigger::player_id` | yes | — |
| `skills_characters` | `skill:高松灯:诗超绊` | `ask_yes,card_settle_at,dist,gain_fire,gate,give_stay,is_placed,log,player_pos,roll,spend_fire,teleport_to,trigger::player_id,trigger::tile,turn_player` | yes | — |
| `skills_characters` | `skill:高松灯（CRYCHIC）:跌跌撞撞...` | `before_turn_end,card_settle_at,gain_fire,give_stay,log,player_pos,skill_blocked,spend_fire,state::add,state::get,trigger::move_roll,trigger::player_id,trigger::value` | yes | — |
| `skills_characters` | `skill:鳰原令王那:梦幻可爱♪女仆` | `add_tok,ask_yes,build_cost,houses_of,in_band,log,owned_tiles,pay_leg,pay_total,player_count,player_out,trigger::player_id` | yes | — |
| `tiles` | `tile:agent` | `agent_offer,ai_agent_choice,ask_tiles,build_cost,buy_quotes,can_build_on,can_pay,houses_of,is_buyable,is_color,log,pay_rent,self_tile,tile_count,tile_group,tile_owner` | yes | — |
| `tiles` | `tile:circle` | `ask_pick,draw,exile_of,field_instances,gain_typed,is_circle,log,player_pos,prop_at,raise,self_tile,set_tile_prop,stun_of,tile_prop` | yes | — |
| `tiles` | `tile:edogawa` | `draw,log,self_tile` | yes | — |
| `tiles` | `tile:event` | `draw,draw_event,log,self_tile` | yes | — |
| `tiles` | `tile:property` | `buy_price,can_build_on,houses_of,log,mortgaged_of,offer_build,offer_buy,offer_force_buy,pay_rent,prop,self_tile,tile_owner` | yes | — |
| `tiles` | `tile:ring` | `` | yes | — |
| `general` | `通用:10次招募（1回限定）` | `draw,pay` | yes | — |
| `general` | `通用:@Tsugu ycm` | `ask_pick,ask_tile,ask_yes,buy_price,cant_move,card_buy,card_move,draw,gain,is_buyable,linger,log,money_of,roll,tile_count,tile_named,tile_owner` | yes | — |
| `general` | `通用:CiRCLE THANKS PARTY!` | `ask_yes,can_pay,gain,give_extra_turn,log,money_of,others,pay,player_out,roll` | yes | — |
| `general` | `通用:GREAT` | `add_to_deck,gain,log,n,set_dest` | yes | — |
| `general` | `通用:[月岛麻里奈]今天也要加油工作喔` | `log` | yes | — |
| `general` | `通用:[衍生]FEVER!` | `field_instances,is_face_down_at,log,n,place_card,set_dest` | yes | — |
| `general` | `通用:[衍生]PERFECT` | `gain,log,set_dest,to_discard` | yes | — |
| `general` | `通用:[都筑诗船]Parking Space` | `add_house,can_build_on,give_stay,houses_of,inc_slot,is_placed,log,owned_tiles,place_card_on,place_mark,self_uid,set_dest,set_slot,slot,tile_named,tile_owner` | yes | — |
| `general` | `通用:安可` | `effect::has,log` | yes | — |
| `general` | `通用:尽力后的收获` | `ask_pick,cant_move,card_move,n,plan::dir,plan::set_resolve,plan::set_steps,tile_steps_ahead` | yes | — |
| `general` | `通用:登上武道馆` | `others,target_all,transfer` | yes | — |
| `general` | `通用:网络链接异常` | `cancel_designation,designations,effect::has,log` | yes | — |
| `general` | `通用:该清CP了` | `add_counter,ask_tile,count_marks,counter,is_placed,log,place_card,placed_cards,player_count,player_out,player_pos,self_uid,send,set_dest,set_slot,slot,tile_count` | yes | — |
| `general` | `通用:雨啊，快点来吧` | `ask_player,give_stay,log,n,others,roll,target` | yes | — |

---

Maintenance: regenerate by grepping `ctx::` in the guest crates and matching
resolved `Op` names against `rules/kani/src/host.rs::dispatch` arms. Keep this
file next to `KANI-FOUNDATION.md`; the foundation doc carries only the summary.

## Hostops-2 (2026-10-11)

`tile_named`, `state_set` packing, `play_doubled`, `trig_set_cancelled` /
`trig_set_negate_effect`, `is_shop`, `neighbor`, stay/stun/exile split,
`add_counter` max, `gain_fire` cap, `shuffle_into_deck`, and
`place_card_on` are now modelled — see `kani/HOSTOPS-ROUND2.md` for the
per-op table and which unit rows those unblock. String-keyed `invoke::find`
is gone from proofs (`card_id!` + `*_idx`); remaining cliffs are `String`/
`Vec` memcmp inside card bodies and move-plan / `rent_of` / name-list ops.
