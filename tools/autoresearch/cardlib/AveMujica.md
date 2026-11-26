# Ave Mujica

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
AM-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.
Explicit non-doll/doll-mode ability blocks have separate scripts; mode changes
alter eligibility without merging their implementations.

## Band Ability

### 假面之下的真实

自己可进入或退出[人偶模式]；未处于[人偶模式]不另设独立状态。

[自动]：

（1）游戏开始时，自己未处于[人偶模式]。自己处于[人偶模式]的第三回合开始时，为此卡添加1个[奇迹水晶]。

（2）自己处于[人偶模式]时：[收取]与[支付]的资金改为1.5倍，只改变自己的收支，对应其他玩家的收支不变。

（3）若自己的技能拥有[火罐]：

- 进入[人偶模式]时：将[火罐]补充至上限。
- 每回合结束时：失去1个[火罐]。
- 回合结束后：若[火罐]为0，退出[人偶模式]。
- 退出[人偶模式]时：失去所有剩余[火罐]。

[主动]：

（1）自己处于[人偶模式]且进行1次[支付]时：可使用此卡的1个[奇迹水晶]，使此次[支付]金额减半。

#### Script allocation

- `AM01AutomaticBandEffect1`: [自动]（1）, initial absence of doll mode and third-turn doll-mode crystal.
- `AM01AutomaticBandEffect2`: [自动]（2）, doll-mode asymmetric money modifier.
- `AM01AutomaticBandEffect3`: [自动]（3）, mode-entry refill, decay, zero-resource exit, and cleanup.
- `AM01ActiveBandEffect1`: [主动]（1）, doll-mode crystal spending for one payment reduction.
- Initial resource/counter values above are shared source-associated metadata, not an extra activated effect.
- Third-turn and turn-end counting remain as written; this pass does not narrow them to own turns.

#### Implementation APIs

- No complete API: implement the [人偶模式] entry/exit lifecycle, crystals, and fire-pot refill/decay. Absence of [人偶模式] is not a separate status. Standard transfer multipliers change both endpoints; the stated owner-only 1.5× accounting requires a separate designed money transformation.

## Characters

### 三角初华 — Imprisoned XII

[资源]：

（1）[火罐]：在[人偶模式]下上限3。

[自动]：

（1）自己未处于[人偶模式]时：移动步数改为10加上[投掷]1d10的结果。

（2）自己处于[人偶模式]时：获得[不可阻挡]。自己进行[主要移动]并[经过]任意玩家时，那名玩家[支付]自己200资金。

[主动]：

（1）自己未处于[人偶模式]，且自己[经过]CiRCLE的回合结束后，自上一次[经过]CiRCLE以来未在任何[回忆地块]（小豆岛、武道馆、旧古河庭园）[结算]时：可在自己的下回合开始时进入[人偶模式]。

（2）自己未处于[人偶模式]并满足此角色[主动]（1）的前述条件时：可在自己[经过]CiRCLE的回合结束后，立刻获得1个新的回合。

（3）自己处于[人偶模式]，每次通过此角色[自动]（2）成功[收取]资金后：可选择在[结算]前额外移动[投掷]1d6的结果所对应的步数。此效果每回合最多使用4次。

#### Script allocation

- `AM02AutomaticAbility1`: [自动]（1）, non-doll-mode movement formula.
- `AM02AutomaticAbility2`: [自动]（2）, doll-mode unstoppable movement and passing-player charges.
- `AM02ActiveAbility1`: [主动]（1）, non-doll-mode memory-history condition and next-own-turn mode entry.
- `AM02ActiveAbility2`: [主动]（2）, optional extra turn under the same memory-history condition.
- `AM02ActiveAbility3`: [主动]（3）, optional post-charge extra movement.
- Initial resource/counter values above are shared source-associated metadata, not an extra activated effect.
- DEC-10.06 confirms [主要移动] for [自动]（2）.
- DEC-10.05 sets the [主动]（3）extra-movement maximum to four uses per turn.

#### Implementation APIs

- Building blocks: `DiceController.ServerTryBeginRoll`, `PlayerRelocationController.ServerTryMovePlayer`, and `PaymentController.ServerTryPlayerPayPlayer`. Missing: state transitions, memory-tile/lap history, extra turns, unstoppable movement, main-roll offset, and bounded extra-movement chaining.
- Pending implementation: track the four-use allowance per turn, not per
  payment or movement chain; a new payment or chain does not reset it.

### 若叶睦 — 天生的演员

[资源]：

（1）[火罐]：初始0，上限6。

[自动]：

（1）自己未处于[人偶模式]，回合开始前：获得1个[火罐]。

（2）自己从手牌使用卡牌时：失去所有[火罐]。

（3）自己进入[人偶模式]时：依次进行以下操作：

1. 将此角色卡换为上一位打出过手牌的玩家的角色卡。
2. 获得新角色的初始[火罐]。
3. 获得1层[睦状态]。

[主动]：

（1）自己未处于[人偶模式]，回合开始前：可使用6个[火罐]，自己[抽卡]1张。

（2）自己未处于[人偶模式]，回合开始时且手牌数为5：可选择进入[人偶模式]。

---

[火罐]：一种通用资源，初始数量、上限及使用方式由对应效果规定。

[抽卡]：从抽卡区抽取卡牌。

[人偶模式]：Ave Mujica团的特有词条，处于该状态时使用对应的人偶模式技能及乐队效果；具体处理见[规则书](../RULEBOOK.md)。

[睦状态]：回合结束时，如果自己的手牌数小于等于1，则退出[人偶模式]并减少1层。

#### Script allocation

- Wording review: the draft replaces six-turn history with fire-pot gain/reset/spending and [睦状态]. The script/API notes below still describe the main text; remapping and replacement/restoration lifetime remain pending acceptance.

- `AM03AutomaticAbility1`: [自动]（1）, non-doll-mode no-hand-play history and draw.
- `AM03AutomaticAbility2`: [自动]（2）, doll-mode character copying, initial resources, duration, and exit.
- `AM03ActiveAbility1`: [主动]（1）, own-turn-start optional mode entry.
- Copied character effects retain their own separate effect scripts and recipient state.
- RES-01: replacement clears the outgoing character-bound pools; incoming fire
  pots start at zero with the incoming cap. This clause then explicitly grants
  the copied character's written initial fire pots; it does not replay game-start
  effects. Restoration is another zero-entry lifecycle, not restoration of a saved
  balance. Player-direct resources survive; admitted effects keep their original context.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards`. Missing: six-turn no-card history, last-card-user history, state transitions, temporary character replacement, original-character restoration, and corresponding initial fire pots.

### 丰川祥子 — 请把你们的人生交给我

[资源]：

（1）[火罐]：初始0，上限3。

[自动]：

（1）自己未处于[人偶模式]，自己的[停留]、[眩晕]、[除外]自然减少层数时：获得1个[火罐]。

（2）自己进入[人偶模式]时：依次进行以下操作：

1. 获得3个[火罐]。
2. 选择至多1张手牌，将其余手牌放入弃卡区。

（3）自己处于[人偶模式]，每回合开始时：使用1个[火罐]，打出抽卡区顶部的1张卡，发动其不带有[反击]的[手]效果。

（4）自己处于[人偶模式]，回合结束后且有0个[火罐]：退出[人偶模式]。

（5）退出[人偶模式]时：失去所有[火罐]。

[主动]：

（1）自己未处于[人偶模式]，回合开始时且至少有3个[火罐]：可选择进入[人偶模式]。

（2）自己未处于[人偶模式]并[经过]其他玩家时：可依次进行以下操作：

1. 那名玩家减少X层[停留]与Y层[眩晕]，X与Y为那名玩家当前的[停留]与[眩晕]层数。
2. 自己[获得](X+Y)×500资金。
3. 自己获得1层[祥子XY]。

---

关键词说明（通用规则见[规则书](../RULEBOOK.md)）：

[火罐]：一种通用资源，初始数量、上限及使用方式由对应效果规定。

[反击]：卡牌响应游戏事件的效果关键词，发动时点由效果文字决定，不豁免状态限制。

[手]：卡从手中打出所触发的效果。

[停留]：具有[无法移动]限制的通用状态，在该玩家的每回合结束前自动减少1层。

[眩晕]：具有[无法移动]、无法使用[主动]技能和手卡的[手]效果、[无法收付款]限制的通用状态，在该玩家的每回合结束前自动减少1层。

[除外]：玩家棋子不视为存在于任何格子上的通用状态，在该玩家的每回合开始前自动减少1层；其余限制及减至0层时的返回规则见规则书。

[经过]：移动实际进入目标时响应，包含实际进入[移动终点]时；移动0格或原地[传送]不触发。

[获得]：直接获得资金，等同于印钱。

[人偶模式]：Ave Mujica团的特有词条，处于该状态时使用对应的人偶模式技能及乐队效果。

[祥子XY]：回合结束前获得X层[停留]与Y层[眩晕]，然后减少1层。

#### Script allocation

- Wording review: the draft changes recharge timing, doll-mode resource/card handling, active numbering and the reward to (X+Y)*500 with delayed [祥子XY]. The script/API notes below still describe the main text. Reconcile these clauses with the band's existing entry refill, turn-end loss and exit cleanup before acceptance; no stacking or override is inferred.

- `AM04AutomaticAbility1`: [自动]（1）, non-doll-mode status-response resource gain; no extra tile settlement.
- `AM04AutomaticAbility2`: [自动]（2）, doll-mode entry hand reduction.
- `AM04AutomaticAbility3`: [自动]（3）, doll-mode automatic top-card play.
- `AM04ActiveAbility1`: [主动]（1）, optional status transfer and per-stack rewards.
- `AM04ActiveAbility2`: [主动]（2）, resource-gated mode entry.
- Unqualified turn-start timing is retained; the current draft normalizes the historical [晕眩] spelling to [眩晕].

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks`, `PaymentController.ServerTryPrintMoney`, and `CardZoneController.ServerTryMoveCardToEnd`. Missing: coordinated status transfer, fire-pot/state transitions, selective hand discard, and authenticated automatic deck-top card play.

### 八幡海铃 — 熟练的支援贝斯手

[资源]：

（1）[火罐]：初始0，上限4。

[自动]：

（1）自己未处于[人偶模式]时，其他玩家可在那名玩家的回合开始时[支付]自己400资金，[指定]自己拥有的1个格子，使该格子立即对前后1格内的1名玩家进行1次半价[结算]，并使付出400资金的玩家[获得]等于自己因此取得金额的资金。直到这两名玩家的下回合结束，若自己仍未处于[人偶模式]，则不会再次通过此效果向他们收费。

（2）自己未处于[人偶模式]，每两个回合没有玩家发动此角色[自动]（1）的服务时：在自己的回合开始时获得1个[火罐]。

[主动]：

（1）自己的回合开始时，若自己未处于[人偶模式]且[火罐]达到上限：可选择进入[人偶模式]。

（2）自己进入[人偶模式]时：可使用X个[火罐]，依次进行以下操作：

1. [指定]其他玩家的X张地契。
2. 对每张被[指定]的地契，向对应玩家[支付]100+n×100资金；n为对应格子的房屋数。
3. 直到[人偶模式]结束，自己与对方均分这些地契[收取]的资金。

每通过此效果使用4个[火罐]，自己[抽卡]1张。

#### Script allocation

- `AM05AutomaticAbility1`: [自动]（1）, non-doll-mode offer to another player, service settlement/reward, and repeat-charge protection.
- `AM05AutomaticAbility2`: [自动]（2）, non-doll-mode unused-service resource recovery.
- `AM05ActiveAbility1`: [主动]（1）, optional mode entry at the resource cap.
- `AM05ActiveAbility2`: [主动]（2）, doll-mode resource spending, deed payments, temporary sharing, and draws.
- The service offer is automatic with another player's processing choice, not an activation by the source owner. The original settlement origin/target geometry and same-player cases remain unresolved.

#### Implementation APIs

- Building blocks: payment transfer/print APIs and `DrawCardController.ServerTryBeginDrawCards`. Missing: remote half-price settlement, participant cooldowns, fire-pot/state transitions, selected-deed revenue sharing, and recording actual received amounts.

### 祐天寺若麦 — 大主播喵梦亲

[资源]：

（1）[火罐]：初始0，上限5。

[自动]：

（1）自己未处于[人偶模式]，每次其他玩家[支付]自己资金时：立即获得1个[火罐]，并使那次[支付]的金额增加X×100；X为自己拥有的[火罐]数量。

（2）自己未处于[人偶模式]，任何时刻[火罐]达到上限时：立即进入[人偶模式]。

（3）自己进入[人偶模式]时：立即将场上全部正面朝上的手卡及事件卡翻为背面朝上。直到[人偶模式]结束，场上所有背面朝上的卡无法产生效果，且不受任何效果影响。自己退出[人偶模式]后，由自己翻面的卡自动翻回。

#### Script allocation

- `AM06AutomaticAbility1`: [自动]（1）, non-doll-mode incoming-payment resource gain and amount increase.
- `AM06AutomaticAbility2`: [自动]（2）, mandatory mode entry at the cap.
- `AM06AutomaticAbility3`: [自动]（3）, doll-mode flips, suppression/protection, and source-specific restoration.

#### Implementation APIs

- Call: `PaymentController.ServerTryAddMoneyAmountDifference` during the matching receipt stage. Missing: fire-pot-triggered state transitions, card/event face-down state, effect suppression/immunity, and restoration limited to cards flipped by this source.

## Cards

### [若叶睦]表演的本能

此卡打出时，效果为场上其他玩家打出的上一张卡的效果。查找上一张卡时，不计入效果带有放置于场上的卡。

此卡复制[反击]卡时，可在符合该[反击]条件时打出。

#### Script allocation

- `AM07PlayEffect`: entire eligible previous-card lookup/copy and reaction-use rule.
  Invoked effects preserve their original identity and script boundaries.

#### Implementation APIs

- No complete API: record the previous eligible other-player card and dispatch its effect through normal timing/target/cost checks. Exclude field-placement effects and preserve counter-card activation eligibility.

### 欢迎来到ave mujica的世界

选择以下效果其一发动：

（1）切换任意一名玩家的[人偶模式]：已处于则退出，未处于则进入（若指定了不支持[人偶模式]的玩家则无效果）。

（2）[反击]当有其他玩家进入或退出[人偶模式]时，你与所有本回合进入或退出过[人偶模式]的玩家同时切换1次[人偶模式]，已处于则退出，未处于则进入；不支持[人偶模式]的玩家不受影响。

#### Script allocation

- `AM08Effect1`: （1）, chosen-player doll-mode entry/exit.
- `AM08ReactionEffect2`: （2）, doll-mode entry/exit reaction and group toggle.
- These are alternative activation modes with distinct timing, not two
  effects automatically executed by a single play.

#### Implementation APIs

- No complete doll-mode toggle API: distinguish support for [人偶模式] from
  currently having that status. Validate mode-capable targets, record this-turn
  entries/exits, and resolve the simultaneous toggle without recursive
  self-trigger loops or granting the mode to unsupported characters.

### 祥，移动

选择1名玩家和移动方向，依次进行以下操作：

1. 强制那名玩家向你选择的方向移动3格。
2. 那名玩家进行[结算]；此次[结算]中进行的[支付]金额减半。

你可在用于确定[主要移动]步数的[投掷]前选择自己，以此次移动代替自己的[主要移动]。

#### Script allocation

- `AM09PlayEffect`: entire forced movement, optional own main-movement substitution,
  settlement, and correlated payment reduction.

#### Implementation APIs

- Calls: `PlayerRelocationController.ServerTryMovePlayer` with signed three steps, `TileResolutionController.ServerTryBeginResolution`, and correlated payment halving. Missing: target/direction choice and self-target main-movement substitution before rolling.

### 燃尽前的线香花火

[资源]：

（1）此卡的[奇迹水晶]：上限2。

将此卡放置于场上，并在此卡上放置2个[奇迹水晶]。

你的回合结束后：自动移除此卡的1个[奇迹水晶]，并使你获得1个额外回合。

最后1个[奇迹水晶]移除后：将此卡置入弃卡区，并立刻使你获得2层[眩晕]。

#### Script allocation

- `AM10PlayEffect`: field placement and two-crystal initialization.
- `AM10FieldEffect`: turn-end decay/extra turns and final discard/stun.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `StatusController.ServerModifyStatusStacks` for two stun stacks. Missing: two-crystal lifetime, extra-turn scheduling, and ordering of the final discard/stun relative to the granted turn.

### 骰子已经掷下

将此卡放置于自身场上。本回合内，所有其他玩家无法从手牌中使用[反击]。回合结束后，将此卡放入弃卡区。

此卡本身可以被[反击]。

#### Script allocation

- `AM11PlayEffect`: field placement; normal reaction eligibility remains.
- `AM11FieldEffect`: other-player hand-reaction restriction and turn-end discard.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for field placement/discard. Missing: turn-scoped prohibition on other players' hand counters, activated only after this card's own counter-response opportunity.

### 无法将视线移开

使当前回合内对你打出过[反击]的所有玩家，向你选择的方向强制移动1至4格中的任意步数，并进行[结算]。

有玩家对你使用[反击]后，你可立即将此卡作为[反击]使用。

若[使用者]在自身回合内选择自己进行强制移动，此次移动视为其[主要移动]。

#### Script allocation

- `AM12PlayEffect`: entire reaction-history targeting, direction/step choices,
  forced settlements, alternative reaction timing, and own-main-movement case.

#### Implementation APIs

- Calls: sequential `PlayerRelocationController.ServerTryMovePlayer` and `TileResolutionController.ServerTryBeginResolution`. Missing: this-turn counter-user history, validated direction/step choices, and clarification of the self-target branch versus the stated target set.

### [三角初华]我，无畏悲伤

打出此卡时，若自从上一次[经过]CiRCLE后，你曾在任何[回忆地块]进行[结算]：可选择以下一个效果：

- 抽1张卡。
- 将本回合用于确定你的移动步数的[投掷]结果，定义为1至6以内的任意数字。

#### Script allocation

- `AM13PlayEffect`: entire memory-history check and draw/dice-result choice.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` for the draw branch. Missing: memory-tile history since CiRCLE, integer choice, and an authorized movement-roll result replacement; a fixed-step plan would change roll semantics.

### [丰川祥子]斩断留恋，忘却一切

抵押1张你拥有且未抵押的最贵地契，并弃置1张手牌；若无手牌，改为弃掉下一张抽到的卡。然后依次进行以下操作：

1. 立刻[传送]至任意可购买或已拥有的格子。
2. 进行[结算]。

此次[传送]作为你的[主要移动]。

若此卡直接从抽卡区打出，你可不弃置手牌发动，或选择不发动此卡。

#### Script allocation

- `AM14PlayEffect`: entire mortgage/card cost, delayed discard if needed,
  teleport settlement, and deck-origin exceptions.

#### Implementation APIs

- Building blocks: `TileController.TryGetMortgageQuote`, `CardZoneController.ServerTryMoveCardToEnd`, and movement-plan setters. Missing: effect-owned chosen mortgage, most-expensive-property validation, deferred next-draw discard, deck-play choice, and main-movement entry.

### 心の雨

使你前方20格内的所有玩家各自[投掷]1d20，并根据各自的结果获得对应效果：

- 结果小于等于3：获得2层[停留]。
- 结果大于3且小于等于12：获得1层[停留]。

若未能使任何玩家获得[停留]，自己获得1层[眩晕]，并获得1000资金。

#### Script allocation

- `AM15PlayEffect`: entire eligible-player rolls, stay outcomes,
  and no-success fallback.

#### Implementation APIs

- Calls: sequential `DiceController.ServerTryBeginRoll` (1d20 per eligible player), `StatusController.ServerModifyStatusStacks`, and conditional `PaymentController.ServerTryPrintMoney` (1000). Track actual successful stay application before deciding the failure branch.

### unnamed

将此卡置于自身场上，并为此卡添加2个[奇迹水晶]。

每回合结束时：移除此卡的1个[奇迹水晶]。此卡的[奇迹水晶]为0时，将此卡置入弃卡区。

在此期间，任意需要失去资金或被其他卡[指定]的时刻：可将此卡置入弃卡区，使自己及前后1格内的其他玩家各获得1层[眩晕]。

#### Script allocation

- `AM16PlayEffect`: field placement and two-crystal initialization.
- `AM16FieldEffect`: decay/expiry and optional discard-to-stun response
  to the stated money-loss or targeting conditions.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `StatusController.ServerModifyStatusStacks` for selected nearby players' stun. Missing: two-crystal lifetime and validated money-loss/card-target timing; applying stun does not inherently cancel the triggering effect.

### 人偶的箱庭

使场上所有其他玩家各自选择以下一个操作执行：

- 进行1次用于确定移动步数的[投掷]，并移动对应步数，不进行[结算]。
- 向你[支付]X×20资金；X为该玩家正常移动到你所在格子所需的移动数。

无法移动的玩家只能选择[支付]。

然后，你强制移动的步数为其他玩家上述[投掷]结果之和。此次移动视为你本回合的[主要移动]。

#### Script allocation

- `AM17PlayEffect`: entire other-player movement/payment choices and subsequent
  user main movement calculated from their recorded rolls.

#### Implementation APIs

- Calls: sequential dice, relocation, and payment-transfer APIs; use a movement plan for the user's resulting main move. Missing: per-player choice, movement eligibility/direction distance, non-settlement integration, and aggregation of accepted roll results.
- S05 `MatchBoardStateData.GetBoardDistance` supplies forward/backward distance;
  eligibility, direction choice, and movement orchestration remain consumer work.

### 会被骗着买水晶的人

将场上1张卡上的1个[奇迹水晶]，移动到另一张可以放置[奇迹水晶]的卡上。

#### Script allocation

- `AM18PlayEffect`: entire legal source/destination selection and crystal transfer.

#### Implementation APIs

- No complete crystal-transfer API: choose two eligible card sources, validate the source balance and destination cap/overflow rules, and commit a coordinated transfer without duplicating or losing the crystal on failure.

### 黑色生日

你以外的每名玩家，根据其资金数额进行对应操作：

- 资金不多于1000：向你[支付]800资金。
- 资金大于1000：向你分别进行2次[支付]，每次200资金。

#### Script allocation

- `AM19PlayEffect`: entire balance-dependent payment processing.
  Preserve the two separate payments for the higher-balance branch.

#### Implementation APIs

- Call: `PaymentController.ServerTryPlayerPayPlayer` for each eligible player: one 800 payment or two sequential 200 payments. Define when balances are sampled; do not collapse the two payments into one because their reactions are distinct.

### [祐天寺若麦]unnamed

（1）将此卡放置于场上。

（2）

1. 每当此卡将从正面翻至背面时：可选择使用2个[火罐]。

2. 每当此卡将从背面翻至正面时：此卡的[拥有者]抽1张卡。

#### Script allocation

- `AM20Effect1`: （1）, field placement.
- `AM20Effect2`: entire （2）, both face-change responses.
  Its internal 1./2. list does not create additional numbered card effects.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `DrawCardController.ServerTryBeginDrawCards`. Missing: card-face transition hooks and fire-pot accounting; clarify what paying two fire pots changes about the pending face-down transition.

### [八幡海铃]unnamed

（1）将此卡放置于行动顺序在[使用者]之后的下一名玩家的场上。[使用者]的回合开始时，将此卡移动到其当前所在场地的玩家之后、行动顺序中的下一名玩家的场上。

（2）[使用者]打出此卡时，以及[使用者]的回合开始时：从卡堆拿取场上有此卡的玩家的所有乐队技能卡。相同乐队技能卡的效果不可叠加，[使用者]不因此视为那个乐队的角色。

（3）当此卡回到[使用者]场上时：在[使用者]的回合结束时，依次进行以下操作：

1. 将此卡与[使用者]拿取的所有乐队技能卡置入弃卡区。
2. 抽1张卡。

#### Script allocation

- `AM21Effect1`: entire （1）, initial other-player placement and turn-start migration.
- `AM21Effect2`: entire （2）, initial/turn-start band acquisition and non-stacking.
- `AM21Effect3`: entire （3）, return condition, card/band cleanup, and draw.
- Share acquired-source references and placement state between these scripts.
  Their same-timing ordering remains governed by the approved resolution rules.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for field-host rotation/discard and `DrawCardController.ServerTryBeginDrawCards`. Missing: non-stacking copied band abilities, turn-order host tracking, character-band identity separation, and cleanup of copied sources.
