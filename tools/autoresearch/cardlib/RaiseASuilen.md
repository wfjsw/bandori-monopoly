# RAISE A SUILEN

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
RAS-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### UNSTOPPABLE

[自动]：

（1）每回合，自己可选择只受到第一次受到的[异常移动效果]。

（2）自己因本技能以外的效果在Live House格子[结算]时：记录该格子，供此卡的[主动]（1）效果检查。

[主动]：

（1）自己的下回合开始时，如果自己仍在此卡[自动]（2）记录的格子上：可在运营阶段选择立刻进入移动阶段并将自己的[主要移动]改为[传送]至自己拥有的1个Live House格子。

#### Script allocation

- `RAS01AutomaticBandEffect1`: [自动]（1）, per-turn abnormal-movement policy with a processing choice.
- `RAS01AutomaticBandEffect2`: [自动]（2）, qualifying settlement history for the next-turn choice.
- `RAS01ActiveBandEffect1`: [主动]（1）, next-turn optional main-movement replacement.
- The original timing for choosing the per-turn policy and handling multiple qualifying settlements remains unresolved.

#### Implementation APIs

- Call: `MovementPlanController.ServerSetMovementPlan` with a teleport plan for the delayed option. Missing: abnormal-movement acceptance limits, settlement provenance, same-tile persistence checks, and owned-Live-House selection.

## Characters

### 和奏瑞依 — 一次又一次竭尽全力

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

[主动]：

（1）自己进行任意[投掷]后：可使用1个[火罐]，再[投掷]1次，并选择两次结果之一执行。

#### Script allocation

- `RAS02AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `RAS02ActiveAbility1`: [主动]（1）, resource cost, reroll, and retained-result choice.

#### Implementation APIs

- Related call: `DiceController.ServerTryBeginRoll`. Missing: fire-pot accounting, reroll continuation after the original operation releases its busy state, result choice, and replacement of the original outcome without duplicate downstream effects.

### 珠手知由 — 天才制作人

[自动]：

（1）自己加盖房屋时：价格减半。如果自己上回合进行过房屋加盖，则本回合加盖房屋免费。

#### Script allocation

- `RAS03AutomaticAbility1`: [自动]（1）, construction discount/free construction and history check.

#### Implementation APIs

- Related calls: `PaymentController.ServerTryMultiplyMoneyAmount` and `ServerTryAddMoneyAmountDifference` for correlated construction payments. Missing: previous-own-turn construction history and an authorized zero-cost construction path consistent with quote validation.

### 朝日六花 — 瞄准目标

[自动]：

（1）自己购买的第一个非“旭汤澡堂”或任意“Live House”格子获得“Live House”的颜色。

（2）自己拥有初始“Live House”格子时：自己拥有的“旭汤澡堂”获得“Live House”的颜色。

#### Script allocation

- `RAS04AutomaticAbility1`: [自动]（1）, first qualifying property's Live House color.
- `RAS04AutomaticAbility2`: [自动]（2）, conditional bathhouse color.
- The original '非…或任意…' first-purchase eligibility remains ambiguous; this pass does not resolve it.

#### Implementation APIs

- No complete API: implement purchase-history-dependent, player-specific Live House color overlays. `TileController.ServerSetTileType` changes the shared tile type and is not a player-specific color modifier.

### 鳰原令王那 — 梦幻可爱♪女仆

[资源]：

（1）[PAREO标记]：初始1，上限3。

[自动]：

（1）游戏开始时，如果场上有Pastel✽Palettes角色：自己额外获得1个正面[P✽P粉丝]。

[主动]：

（1）自己拥有格子上的房屋总数增加时：可使用1个[PAREO标记]，所有其他玩家[分摊][支付]自己X资金。X为自己拥有的所有格子中“格子的房屋造价×格子上的房屋数”的最高值的四分之一。

#### Script allocation

- `RAS05AutomaticAbility1`: [自动]（1）, conditional initial fan.
- `RAS05ActiveAbility1`: [主动]（1）, house-count trigger, marker cost, maximum valuation, and shared payment.
- Initial resource/counter values above are shared source-associated metadata, not an extra activated effect.

#### Implementation APIs

- Call: `PaymentController.ServerTryPlayerPayPlayer` with the calculated total shared across other players. Missing: initial fan grant, PAREO markers, owned-house increase detection, and maximum property-construction valuation.

### 佐藤益木 — 与燃烧的红色一起驰骋

[资源]：

（1）[火罐]：初始0，上限3。

[自动]：

（1）自己每次领取完CiRCLE奖励后首次[经过]“白雪学园”或“银河拉面馆”之一时：获得1个[火罐]。

[主动]：

（1）自己的移动阶段前：可使用X个[火罐]，为本回合用于确定[主要移动]步数的[投掷]额外添加X个d20骰子。

#### Script allocation

- `RAS06AutomaticAbility1`: [自动]（1）, named-tile fire pots and initial amount/cap.
- `RAS06ActiveAbility1`: [主动]（1）, resource choice and additional movement dice.

#### Implementation APIs

- Call: `MovementPlanController.ServerModifyMovementDice` to add X d10 during the owner's BeforeDiceRoll window. Missing: fire-pot recharge/cost choice and carrying a pre-movement activation into that permitted modification window.

## Cards

### R. I. O. T.

[反击]你被其他玩家的卡效果影响时：打出此卡。所有玩家将各自的全部手牌放入弃卡区，并各自抽等于其弃卡数量的卡。你额外抽1张卡。

#### Script allocation

- `RAS07PlayEffect`: entire reaction, all-player hand discard/redraw, and extra draw.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for each discarded hand card and sequential `DrawCardController.ServerTryBeginDrawCards`. Snapshot each player's count; define whether all discards precede all draws and how the triggering card is counted.

### EXIST

将此卡放置于自己的场上。直到你的下回合开始，场上及打出的所有对单一玩家生效的手卡，其目标改为你，包括其他玩家指向自身的卡。

你的下回合开始时：将此卡翻入弃卡区。若在此期间此卡没有造成影响，抽1张卡。

#### Script allocation

- `RAS08PlayEffect`: field placement and activation-context initialization.
- `RAS08FieldEffect`: eligible target redirection, expiry/discard,
  and no-impact draw using this source's recorded outcomes.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and conditional `DrawCardController.ServerTryBeginDrawCards`. Missing: pending single-target hand-effect redirection, self-target compatibility, affected-effect tracking, and next-turn expiration.

### 狂乱Hey Kids!!

[反击]在属于你的格子上[结算]时：打出此卡，将本次[结算]改为：将本格上的房屋转移到属于你的可建造格子上。转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋。

转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；

随后，你失去“转移后各格房屋造价总和－获得房屋数量×500”的资金。

#### Script allocation

- `RAS09PlayEffect`: entire settlement replacement, legal house redistribution,
  value balancing, and final money loss.

#### Implementation APIs

- Building blocks: `PaymentController.ServerTryPrintMoney` and `ServerTryDeleteMoney`. Missing: custom settlement replacement, validated house redistribution/demolition, value conservation, rent refresh, and clarification of the stated transfer-count constraint.

### Change the world

将此卡放置于你的1个有房屋的Live House格子上。本次用于确定你的移动步数的[投掷]改为3d20。

此次移动期间，每[经过]1个不属于你的Live House格子，为此卡添加1个[奇迹水晶]。此地块的下一次收费增加50×（y+1）×n资金，且触发时获得等量资金；y为[奇迹水晶]数量，n为此卡放置格子上的房屋层数。触发后，将此卡放入弃卡区。

#### Script allocation

- `RAS10PlayEffect`: chosen Live House placement and this-movement dice replacement.
- `RAS10FieldEffect`: qualifying pass crystals, next-charge increase/reward,
  and discard. Record the movement and charge this activation refers to.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `MovementPlanController.ServerSetMovementPlan` with 3d20, and payment additive/print operations. Missing: path-counted crystals, next-rent correlation, and a defined meaning for the additional equal money gain.

### 成为最强

[投掷]1d10，[传送]到Live House对应的格子，按格子编号排序。若结果为10或以上，或你没有Live House格子，改为[传送]到“Live House”。

此次[传送]视为你的[主要移动]。

#### Script allocation

- `RAS11PlayEffect`: entire roll, ordered Live House destination/fallback,
  and teleport main movement.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (1d10) and `MovementPlanController.ServerSetMovementPlan` with a teleport plan. Missing: ordered Live House lookup, fallback handling, and main-movement entry.

### PLEASE CHOOSE

[反击]有玩家在Live House格子上[结算]时：使对方选择以下一个效果执行：

- 立即打出1张可将你[指定]为目标的卡，并将你[指定]为目标之一。
- 使你立即[传送]至对方所在格子，不触发[结算]，但视为可触发乐队技能。

#### Script allocation

- `RAS12PlayEffect`: entire settlement reaction and other-player choice.
  （1）/（2）are alternative outcomes inside this effect, not independent plays.

#### Implementation APIs

- Call: `PlayerRelocationController.ServerTryTeleportPlayer` for the teleport branch without explicit settlement. Missing: another player's forced card-play choice/targeting and synthetic band-trigger provenance despite the absence of settlement.

### 练习室里的风暴

[资源]：此卡的[奇迹水晶]初始0，上限3。

（1）你位于自己拥有的Live House格子上时：可将此卡放置在当前格子上。每次[使用者][经过]CiRCLE时，为此卡放置1个[奇迹水晶]。

（2）[使用者]以外的玩家在距此卡所在格子X格处[结算]时，若X至少为1且不大于此卡的[奇迹水晶]数量：将此卡放入弃卡区，并对那名玩家进行1次收费，金额为此卡所在格子普通[结算]价格的(4-X)/4倍。

#### Script allocation

- `RAS13Effect1`: entire （1）, placement eligibility and capped pass crystals.
- `RAS13Effect2`: entire （2）, distance/crystal eligibility, discard, and scaled charge.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `PaymentController.ServerTryPlayerPayPlayer`. Missing: crystals, distance/rent quote selection, special-property valuation, and source-correlated remote charging before discarding.
- S05 `MatchBoardStateData.GetBoardDistance` supplies the distance calculation;
  property selection, rent valuation, and resource/payment integration remain Pending.

### 游击演出

[特]：

此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家[支付]了至少1000资金且你不拥有任何可盖房的Live House格子且场上已不存在可购买的此类格子，则可选择将此卡置于场上，[指定]你拥有的1张最贵地契，使其对你视为Live House格子。

[手]：

[反击]当你[经过]属于其他玩家的Live House格子后，且当次[结算]时被其他玩家的格子收取资金后：依次进行以下操作：

1. [传送]至任意无主可购买的格子。
2. 进行[结算]，且必须购买。

#### Script allocation

- `RAS14SpecialEffect`: entire [特] discard-turn condition, field placement,
  and the selected property's Live House trait.
- `RAS14HandEffect`: entire [手] reaction, teleport, settlement, and mandatory purchase.

#### Implementation APIs

- Calls: `PlayerRelocationController.ServerTryTeleportPlayer`, `TileResolutionController.ServerTryBeginResolution`, and `CardZoneController.ServerTryMoveCardToEnd`. Missing: mandatory purchase orchestration, payment/pass history, discard-turn trigger, and player-specific Live House conversion.

### UNSTOPPABLE

[投掷]1d6mod6，根据结果[传送]至对应格子：

- 结果为1：白雪学园。
- 结果为2：艺术学院高中。
- 结果为3：瑟罗希亚国际学校。
- 结果为4：银河拉面馆。
- 结果为5：旭汤澡堂。
- 结果为6：CHUCHU的公寓。

此次[传送]不触发[结算]，视为你的[主要移动]。结果为1至3时，获得2000资金；结果为4至6时，获得1000资金。

#### Script allocation

- `RAS15PlayEffect`: entire card roll, destination, teleport main movement,
  and result-dependent money. This is distinct from the band's same-named source.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (1d6), movement-plan setters for a non-settling teleport, and `PaymentController.ServerTryPrintMoney`. Missing: main-movement entry; resolve modulo-zero versus the listed 1–6 outcomes.

### Repaint

[反击]任意其他玩家进行用于确定移动步数的[投掷]并进入移动阶段后：打出此卡，使目标玩家此次移动步数减少X；X为对方原本预计路径上你拥有的格子数。对方此次[结算]中的[支付]金额减半。

#### Script allocation

- `RAS16PlayEffect`: entire movement reaction, original-path property count,
  movement reduction, and correlated settlement-payment reduction.

#### Implementation APIs

- Related call: `PaymentController.ServerTryMultiplyMoneyAmount` (1/2) for the correlated settlement. Missing: original-path snapshot, active movement-distance reduction, zero/negative-result policy, and settlement-specific modifier lifetime.
- S05 `MatchBoardStateData.GetPotentialMovementPathSnapshot` can query a captured
  origin and signed step count (at most 255 entries). It does not capture an active
  operation's original path or alter that operation.

### [珠手知由]演奏我的音乐吧

将此卡放置于你以外1名玩家的场上，并为此卡添加3个[奇迹水晶]。那名玩家的每个回合结束时，失去此卡的1个[奇迹水晶]。

场上存在此卡的玩家下次购买地契时，依次进行以下操作：

1. [使用者]获得100资金。
2. 将此卡移至除[使用者]外行动序列下一名玩家的场上，并将此卡的[奇迹水晶]补充至3个。

此卡进入弃卡区前，每触发1次上述效果，此卡获得资金时额外获得100资金，上限500。

此卡的[奇迹水晶]为0时：将此卡放入[使用者]的弃卡区，并使[使用者]抽1张卡。

#### Script allocation

- `RAS17PlayEffect`: other-player field placement and initial crystals.
- `RAS17FieldEffect`: decay, purchase rewards, next-player migration/refill,
  reward growth, and exhaustion discard/draw.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryPrintMoney`, and `DrawCardController.ServerTryBeginDrawCards`. Missing: field-host versus owner tracking, three-crystal lifetime, purchase-trigger handoff, and capped escalating reward.

### [朝日六花]追逐梦想的步伐

（1）抽取游戏开始的2张手牌前：将此卡从卡组展示给所有玩家，并放置在自身场上。游戏开始时，[传送]到“东京外”，并获得3层[除外]。

（2）若此卡[拥有者]的[主要移动][经过]了“Bandori车站”：在[结算]前，将[移动终点]改为“旭汤澡堂”，然后[移除]此卡。

#### Script allocation

- `RAS18Effect1`: entire （1）, pre-hand setup, reveal/placement, teleport, and banishment.
- `RAS18Effect2`: entire （2）, path-dependent destination replacement and removal.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PlayerRelocationController.ServerTryTeleportPlayer`, `StatusController.ServerModifyStatusStacks`, and `CardRemovalController.ServerRemoveCard`. Missing: pre-hand setup/reveal ordering and replacement of the active main-movement destination.

### [佐藤益木]CRUSH ON THE DRUM!!!

[特]：

打出此卡的回合内，[使用者]使用的任何包含添加骰效果的卡，不受任何其他效果影响。

[手]：

移动阶段前打出此卡，为本回合用于确定[主要移动]步数的[投掷]额外添加X个d20骰子；X为你弃卡区的卡数。

#### Script allocation

- `RAS19SpecialEffect`: [特], this-turn protection of qualifying dice-adding cards.
- `RAS19HandEffect`: entire [手] timing, discard-count calculation, and extra dice.

#### Implementation APIs

- Call: `MovementPlanController.ServerModifyMovementDice` to add X d20 in BeforeDiceRoll. Missing: discard-count sampling at activation and turn-scoped protection specifically for the user's dice-adding cards.

### [鳰原令王那]渐渐远去的你

依次进行以下操作：

1. 获得2个[PAREO标记]。
2. 视为你的房屋总数增加，且可选择移除任意1个你拥有的格子上的1层房屋。

#### Script allocation

- `RAS20PlayEffect`: entire marker gain, simulated house-count increase,
  and optional house removal.

#### Implementation APIs

- No complete API: grant two PAREO markers, dispatch the character's house-increase behavior without actually adding a house, and optionally remove one eligible house with rent refresh.

### [和奏瑞依]寄于指尖的执念

[反击]你使用[火罐]进行[投掷]时：可打出此卡，保留并写下未被选择的另一个结果。可保留多个结果。

在后续任意回合中，可使用1个[火罐]，将保留的结果用于替代当回合用于确定移动步数的[投掷]结果，随后删去该保留结果。

#### Script allocation

- `RAS21PlayEffect`: entire reroll reaction, storage of the unselected result,
  and registration of later fire-pot-funded movement-result replacement.
- Stored results and their spending behavior must persist independently
  of this card's zone; no field placement is stated.

#### Implementation APIs

- No complete API: retain the unchosen character-reroll result, support multiple stored results, and replace a future movement roll after fire-pot payment. Stored plan totals alone do not implement roll replacement and provenance.
