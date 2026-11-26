# Morfonica

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
MO-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 振翅高飞的练习曲

[自动]：

（1）自己一次性失去2000以上资金时：自己[抽卡]1张。

（2）自己领取[CiRCLE奖励]并选择[获得]资金时：根据选择[获得]资金的次数，金额依次按1000、1500、2000循环。

#### Script allocation

- `MO01AutomaticBandEffect1`: [自动]（1）, qualifying money-loss draw; retains the original threshold wording.
- `MO01AutomaticBandEffect2`: [自动]（2）, cyclic CiRCLE money rewards.

#### Implementation APIs

- Calls: `DrawCardController.ServerTryBeginDrawCards` after a qualifying actual loss and `PaymentController.ServerTryPrintMoney` for the selected reward. Missing: single-loss grouping and a cycling replacement for the normal CiRCLE money reward.

## Characters

### 仓田真白 — 向后全速前进

[自动]：

（1）自己用于确定移动步数的[投掷]改为2d20

（2）自己进行移动前：使此次移动改为反向。

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MO02AutomaticAbility1`: [自动]（1）, movement dice replacement.
- `MO02AutomaticAbility2`: [自动]（2）, movement reversal.
- DEC-10.06 confirms that [自动]（1）applies to [主要移动].

#### Implementation APIs

- Call: `MovementPlanController.ServerSetMovementPlan` with a 2d20 dice plan. Missing: default-plan integration and reversed main-movement direction; configure before the plan is locked rather than replacing an already active move.

### 桐谷透子 — 天上天下，唯我独尊

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。

[主动]：

（1）自己的经营阶段：可使用1个[火罐]，[指定]自己拥有的1个格子。该格子前后各1格范围内（不包括该格子本身）的所有其他玩家被[指定]，各[支付]自己X资金。X为该格子的地租的一半÷被[指定]玩家数量（向上取整百）。每回合限1次。

#### Script allocation

- `MO03AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `MO03ActiveAbility1`: [主动]（1）, cost, property/player targeting, payment formula, and usage limit.

#### Implementation APIs

- Calls: `TileController.TryCalculateRentAmount` where applicable and `PaymentController.ServerTryPlayerPayPlayer` with total X × selected payer count. Missing: fire pots, once-per-turn state, owned-tile choice, special-rent valuation, and zero-target handling.

### 广町七深 — 这是很普通的事吧？

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]（初始2，上限2）。

[主动]：

（1）有玩家使用角色技能时：自己可使用1个[火罐]，获得1个记录该角色的[模仿标记]。场上[存活]玩家数量至少为3时，每种被模仿角色对应的[模仿标记]上限1，且此技能对[自动]技能存在3回合冷却。

（2）时机合适时：自己可使用持有的[模仿标记]，视为使用该标记记录的角色的技能。若该技能需要使用[火罐]，则使用自己拥有数量与所需数量中的较小值；其他所需资源（例如资金和翻转[P✽P粉丝]）仍须按该技能的要求使用。

#### agent proposed wording change

[资源]：

（1）[火罐]：初始2，上限2。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

[主动]：

（1）有玩家使用角色技能时：自己可使用1个[火罐]，获得1个记录该角色的[模仿标记]。场上[存活]玩家数量至少为3时，每种被模仿角色对应的[模仿标记]上限1，且此技能对[自动]技能存在3回合冷却。

（2）时机合适时：自己可使用持有的[模仿标记]，视为使用该标记记录的角色的技能。若该技能需要使用[火罐]，则使用自己拥有数量与所需数量中的较小值；其他所需资源（例如资金和翻转[P✽P粉丝]）仍须按该技能的要求使用。

#### human verified wording change

#### word edit status

draft

#### Script allocation

- `MO04AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `MO04ActiveAbility1`: [主动]（1）, resource cost, [模仿标记], cap, and cooldown.
- `MO04ActiveAbility2`: [主动]（2）, marker spending and copied ability invocation.
- [模仿标记] is a character status, not a resource or hand card. Applying it
  adds the corresponding skill effect to the affected character's effect source;
  preserve represented-character identity and the copied skill's conditions.
- Acquiring [模仿标记] costs one [火罐]. Invoking the copied skill
  separately spends min(owned, needed) [火罐]; other resource requirements
  still apply. Do not grant missing resources.
- The scope of the three-turn cooldown remains unresolved.

#### Implementation APIs

- No complete API: implement [模仿标记] status acquisition, per-character
  caps/cooldowns, and copied-skill dispatch. Apply the accepted fire-pot
  policy above without waiving other resource costs, timing or target checks.

### 八潮瑠唯 — 正论暴击

X初始为0。

[自动]：

（1）自己的[自动]（2）技能或角色卡被复制后，复制者将X重置为0时：那名玩家失去该技能。

（2）自己即将获得或失去资金时（抵押或赎回除外），在所有其他资金量改动效果生效后、结果不等于0时：[投掷]1d20，并根据结果进行以下操作：

1. 结果小于等于X：将X重置为0，此次获得的资金翻倍或无需失去资金，收款或付款方同样受到影响；

2. 结果大于X：X增加1。

#### agent proposed wording change

[资源]：

（1）X：初始0。

[自动]：

（1）自己的[自动]（2）技能或角色卡被复制后，复制者将X重置为0时：那名玩家失去该技能。

（2）自己即将获得或失去资金时（抵押或赎回除外），在所有其他资金量改动效果生效后、结果不等于0时：[投掷]1d20，并根据结果进行以下操作：

1. 结果小于等于X：将X重置为0，此次获得的资金翻倍或无需失去资金，收款或付款方同样受到影响；

2. 结果大于X：X增加1。

#### human verified wording change

#### word edit status

draft

#### Script allocation

- `MO05AutomaticAbility1`: [自动]（1）, copied-ability removal on counter reset.
- `MO05AutomaticAbility2`: [自动]（2）, late money-stage roll, counter update, and two-sided money outcome.
- Initial resource/counter values above are shared source-associated metadata, not an extra activated effect.
- Generic money gain/loss is retained rather than narrowed to bank-only [获得]/[消耗].

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` and the matching payment multiply/cancellation APIs. Missing: reaction-stage ordering, X history, mortgage/redemption exclusion, and copied-skill revocation; cancellation must occur in an authorized cancellation stage.

### 二叶筑紫 — 交给班长吧

[自动]：

（1）场上除自己以外的玩家均拥有1枚翻面后的[筑紫标记X]时：移除所有[筑紫标记X]，自己[获得]移除标记数量×800资金。每次发动此[自动]（1）效果时，单个标记的收益减少200（最低200）。

[主动]：

（1）自己获得资金或[抽卡]时：可改为[指定]场上除自己以外的1名角色进行1次该动作，并给予那名角色1个[筑紫标记X]。X记录因此效果获得的资金数量；若为[抽卡]，则记录2200。单轮结算（例如“登上武道馆”的结算）中，此效果最多发动1次。

（2）有角色获得资金或[抽卡]时：自己可将那名角色的[筑紫标记X]翻面，代替其进行1次该动作。以此效果获得的资金不能超过记录金额加300，超出部分由原角色获得。替代[抽卡]时，必须翻面记录金额至少为2200的标记。

#### agent proposed wording change

[资源]：

无。

[自动]：

（1）场上除自己以外的玩家均拥有1枚翻面后的[筑紫标记X]时：移除所有[筑紫标记X]，自己[获得]移除标记数量×800资金。每次发动此[自动]（1）效果时，单个标记的收益减少200（最低200）。

[主动]：

（1）自己获得资金或[抽卡]时：可改为[指定]场上除自己以外的1名角色进行1次该动作，并给予那名角色1个[筑紫标记X]。X记录因此效果获得的资金数量；若为[抽卡]，则记录2200。单轮结算（例如“登上武道馆”的结算）中，此效果最多发动1次。

（2）有角色获得资金或[抽卡]时：自己可将那名角色的[筑紫标记X]翻面，代替其进行1次该动作。以此效果获得的资金不能超过记录金额加300，超出部分由原角色获得。替代[抽卡]时，必须翻面记录金额至少为2200的标记。

#### human verified wording change

#### word edit status

draft

#### Script allocation

- `MO06AutomaticAbility1`: [自动]（1）, all-other-player condition, marker cleanup, and declining reward.
- `MO06ActiveAbility1`: [主动]（1）, delegated money/draw, marker record, and resolution limit.
- `MO06ActiveAbility2`: [主动]（2）, marker flip, capped replacement money/draw, and original-recipient remainder.
- [筑紫标记X] is a status, not a resource. Preserve the amount X recorded
  by the skill, the marker's face state, and source identity.
  The scope of generic money receipts is unchanged.

#### Implementation APIs

- Building blocks: `PaymentController.ServerTryPrintMoney` and `DrawCardController.ServerTryBeginDrawCards`. Missing: replacing pending recipients/draws, per-resolution use limits, face-flipping recorded markers, capped split outcomes, and declining collection rewards.

## Cards

### 迷茫之蝶们的三全音

[反击]任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金。

#### Script allocation

- `MO07PlayEffect`: money-loss reaction, immediate advance, placement, and crystals.
- `MO07FieldEffect`: crystal countdown, discard, and repayment of the recorded advance.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryPrintMoney`, and a later money operation. Missing: safe credit during a pending payment, stored debt/three-turn lifetime, and clarification of who receives the later payment.

### 夏日合宿

打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌。

#### Script allocation

- `MO08PlayEffect`: entire timed targeting protection and expiry-dependent draw.
  The duration is an activation-scoped grant, not an unstated field card.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` if no protected targeting occurred. Missing: source-aware target rejection, expiry before the next turn, and reliable tracking of effects prevented by this protection.

### 你的光芒将照亮前路

当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡，此次移动以“月之森女子学院”为起点（不触发起点地块效果）。

#### Script allocation

- `MO09PlayEffect`: entire distance eligibility and silent movement-origin replacement.

#### Implementation APIs

- No complete API: validate the distance condition and replace the upcoming movement origin without tile/pass/landing effects. Normal `PlayerRelocationController.ServerTryTeleportPlayer` emits events and is not a silent origin setter.
- S05 `MatchBoardStateData.GetBoardDistance` supplies distance validation;
  silent movement-origin replacement remains Pending.

### 蝴蝶飞舞的星月夜

（1） 支付X次1000的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]，此卡没有[奇迹水晶]时加入弃卡区; 此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次。

（2）当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]，此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。

（3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。

#### Script allocation

- `MO10Effect1`: entire （1）, payment/placement, crystals, exhaustion, and reroll option.
- `MO10Effect2`: entire （2）, other-player payment option and CiRCLE crystal removal.
- `MO10Effect3`: （3）, reward for each crystal removed.

#### Implementation APIs

- Calls: sequential `PaymentController.ServerTryDeleteMoney`, `ServerTryPrintMoney`, and `DiceController.ServerTryBeginRoll`. Missing: central-field placement, crystal lifecycle, optional movement reroll replacement, and cross-player trigger/payment ordering.

### 勇气展翅高飞之时

掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半，若为地产商地块，获得1000资金。

#### Script allocation

- `MO11PlayEffect`: entire roll and property-owner payment/developer reward branch.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (3d20), `PaymentController.ServerTryPlayerPayPlayer`, or `ServerTryPrintMoney` (1000). Missing: multi-owner payment allocation and an agreed property-plus-construction valuation formula.

### [广町七深]稍微努力了一下

移除自己持有的x个[模仿标记]，发动以下效果中的一个：

（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张。

（2）获得x次经过CiRCLE时的资金奖励。

（3）将此卡放置在场上并放置x个奇迹水晶，你可以使用1个[奇迹水晶]视为发动你的[主动]（1）技能，此次技能不受数量或轮数限制。

（4）当奇迹水晶耗尽时，将此卡置入弃卡区。

#### Script allocation

- `MO12Effect1`: （1）, chosen draw outcome and turn-end hand reduction.
- `MO12Effect2`: （2）, chosen repeated CiRCLE money rewards.
- `MO12Effect3`: （3）, chosen field placement/crystals and later [主动]（1） ability substitution.
- `MO12Effect4`: （4）, exhaustion discard while the field outcome is active.
- Share the opening marker cost and choice through one activation context;
  effects 1–3 are alternatives, not three independent plays. The placement
  of （4）inside the written alternatives is ambiguous; do not offer cleanup
  as a standalone selectable benefit.

#### Implementation APIs

- Calls: `DrawCardController.ServerTryBeginDrawCards`, `PaymentController.ServerTryPrintMoney`, and `CardZoneController.ServerTryMoveCardToEnd`. Missing: [模仿标记] status removal, hand-limit bypass/deferred discard, repeated CiRCLE reward semantics, crystals, and copied-skill limit overrides.

### [桐谷透子]unnamed

选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回。

#### Script allocation

- `MO13PlayEffect`: entire Toko property selection, exchange, value adjustment,
  and redemption sequence.

#### Implementation APIs

- Related call: `PaymentController.ServerTryPlayerPayPlayer` for the difference. Missing: validated two-deed exchange, coordinated ownership/mortgage transfer, free redemption, and transactional failure handling; owner add/remove calls alone do not supply an atomic exchange.

### [仓田真白]unnamed

[反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金。

#### Script allocation

- `MO14PlayEffect`: entire Mashiro payment reaction and replacement money losses.

#### Implementation APIs

- Building blocks: `PaymentController.ServerTryRequestMoneyChangeCancellation` and `ServerTryDeleteMoney` for the replacement losses. Missing: replacing a pending transfer and sequencing two losses after cancellation without reopening the active money pipeline.

### 离心力，不为所动

[反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果。

#### Script allocation

- `MO15PlayEffect`: entire second-targeting reaction and timed protection;
  targeting history is queried without requiring this card to have been played earlier.

#### Implementation APIs

- No complete API: track distinct incoming targeting since the previous own-turn end, activate at the second eligible occurrence, and suppress all qualifying effects until the next own-turn start.

### 高贵的微蓝

当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置1个[微蓝标记]，有此标记时该地块不能被[指定]。[微蓝标记]永久保留，不随回合推进或来源卡离场而消失；明确移除该标记的效果仍按自身规则处理。

#### Script allocation

- `MO16PlayEffect`: entire location/ownership check, settlement, and protection-marker grant.
  The inline 1./2. operations are steps; marker protection belongs to tile state.

#### Implementation APIs

- Call: `TileResolutionController.ServerTryBeginResolution` for immediate settlement after eligibility checks. Missing: persistent tile protection/markers and target rejection; clarify the outcome when the qualifying tile is not owned by the user.

### 再次牵起手来

[手]：

[反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金。

[持续]：

[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。

#### Script allocation

- `MO17HandEffect`: entire [手] reaction, placement, and matched expenditure.
- `MO17FieldEffect`: entire [持续] money cancellation and discard.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryDeleteMoney`, and `ServerTryRequestMoneyChangeCancellation`. Missing: predecessor/actual-loss attribution and safe post-payment sequencing; discard only when the intended cancellation is accepted.

### 纯真振翅

传送到移动方向20格后（不触发结算），立刻进行移动掷骰。

#### Script allocation

- `MO18PlayEffect`: entire teleport without settlement followed by movement roll.

#### Implementation APIs

- Calls: `PlayerRelocationController.ServerTryTeleportPlayer` and subsequently `DiceController.ServerTryBeginRoll`. Missing: current-direction lookup and integration of the new roll into main movement; teleport does not itself perform settlement.

### 秘密与青春的虹彩

[反击]（1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。（2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。

#### Script allocation

- `MO19ReactionEffect1`: （1）, qualifying outgoing-payment reduction.
- `MO19ReactionEffect2`: （2）, qualifying incoming-payment increase.

#### Implementation APIs

- Call: `PaymentController.ServerTryMultiplyMoneyAmount` during
  `PlayerPayPlayerPostSplitMultDiff`, with the current operation ID and exact
  concrete payer/payee IDs. Effect (1): the card player is the payer, the payee
  is a junior or same-year peer, and the multiplier is 1/2. Effect (2): the card
  player is the payee, the payer is a senior or same-year peer, and the multiplier
  is 3/2. Determine seniority from current characters; preserve other activation
  conditions. Round upward to ten immediately after each multiplier; both
  endpoints use the adjusted amount and other split entries remain unchanged.
  Do not use before-payment or cancellation stages for these effects.
  Missing: authoritative school-year relationship lookup and runtime validation.

### [八潮瑠唯]正论恶魔

向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，每当你的技能被触发后，立即将x设置为5。

#### Script allocation

- `MO20PlayEffect`: entire ordered payment sequence and the activation-scoped
  X-reset rule after each relevant ability trigger.

#### Implementation APIs

- Related call: `PaymentController.ServerTryPlayerPayPlayer` for the return payments. Missing: an unmodifiable player-to-player transfer, exact per-player versus shared amounts, and effect-scoped X resets; print/delete-without-events is not an equivalent transfer API.

### [二叶筑紫]迷茫的庭园

获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动。

#### Script allocation

- `MO21PlayEffect`: entire reward, roll/player choice, movement/settlement choice,
  and timed abnormal-movement protection.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney` (100), `DiceController.ServerTryBeginRoll` (1d6), and movement-plan setters. Missing: action-order destination choice, optional settlement, main-movement entry, and temporary abnormal-movement immunity.
