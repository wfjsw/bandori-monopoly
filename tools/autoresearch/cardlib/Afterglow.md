# Afterglow

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
AG-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 商店街的宠儿

[自动]：

（1）购买商店街的格子或价值小于等于1200的格子时：免费在该格子加盖1栋房屋。

#### Script allocation

- `AG01AutomaticBandEffect1`: [自动]（1）, qualifying purchase and free construction.

#### Implementation APIs

- Calls: `TileController.ServerDevelopTile` followed by `ServerSetDefaultRent` for authorized free construction. Validate ownership, eligibility, cap, and transaction timing first; define which purchase-price basis determines the 1200 threshold.

## Characters

### Shared character API usage

- The original character ability （2） is [主动]（1） after category-local
  numbering. All five recharge abilities refer to that category-qualified
  ability, including when a different Afterglow character's ability is used.
- Keep the initial one [火罐], cap of one, recharge counter, and ability-use
  history in shared authoritative character/source state, not duplicated
  across effect scripts. Four-turn counting/reset details must be authored;
  do not silently count every player's turn or invent a repeated-grant rule.
- Validate activation timing, resource availability and the relevant movement
  state on the server. Select inputs at the relevant step's beginning and
  revalidate before applying changes.
- CTC-06 resource operations, CTC-08 movement extensions and CTC-14 scheduling/
  history operations below are proposed requirements, not callable methods.
- Existing asynchronous calls return admission separately from completion.
  Use the parent-effect overload during effect resolution and handle rejection
  and completion exactly once. Do not substitute raw position writes for a
  coordinated movement operation.
- Validation: API signatures/source inspected; Unity/runtime checks Not Run.

### 美竹兰 — 叛逆的红挑染

Band: Afterglow. Color: `#EE0022`.

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。

[主动]：

（1）自己进行用于确定本次移动步数的[投掷]前：可使用1个[火罐]，选择本次移动反向。因该技能的效果反向移动[经过]CiRCLE时，不获得[CiRCLE奖励]。

#### Script allocation

- `AG02AutomaticAbility1`: [自动]（1）, four-turn recovery, initial amount/cap; references the original main skill, now [主动]（1）.
- `AG02ActiveAbility1`: [主动]（1）, backward movement choice, cost, and reward suppression.

#### Implementation APIs

- Sequence: validate the pre-movement-roll window, spend one [火罐], and record
  backward direction plus CiRCLE-reward suppression for this movement.
- Existing building block: `PlayerRelocationController.ServerTryMovePlayer(
  parentEffect, playerId, spaces, onOperationResolved, onChainFinished)`
  accepts signed steps. It does not itself configure the normal main movement.
- `MovementPlanController.ServerSetDiceTotal(playerId, diceTotal)` rejects
  negative totals; do not use a negative dice total as a direction flag.
- Missing: CTC-06 resource accounting, CTC-08.04 direction integration,
  CTC-09.06 source-scoped CiRCLE reward suppression, and CTC-14.06 ability-use
  history/recharge. Preserve pass events; suppress only the stated reward.

### 青叶摩卡 — 我行我素

Band: Afterglow. Color: `#00CCAA`.

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。

[主动]：

（1）其他玩家使用角色技能或打出手牌时：自己可使用1个[火罐]，[传送]到那名玩家所在的格子。此次[传送]不触发[结算]，且[传送]本身不引发其他被动效果（如玩家[重叠]）。

#### Script allocation

- `AG03AutomaticAbility1`: [自动]（1）, four-turn recovery, initial amount/cap; references the original main skill, now [主动]（1）.
- `AG03ActiveAbility1`: [主动]（1）, reaction teleport, cost, and passive-effect suppression.

#### Implementation APIs

- Sequence: validate the other player's character-ability/hand-play event,
  select that player's tile at the teleport step, validate/spend one [火罐],
  and request a non-settling teleport with the required passive suppression.
- Existing building block: `PlayerRelocationController.ServerTryTeleportPlayer(
  parentEffect, playerId, destinationTileIndex, onOperationResolved,
  onChainFinished)`. It emits relocation events and has no suppression argument;
  it cannot satisfy this ability unchanged.
- Missing: CTC-08.09 scoped trigger policy, character/card-use event integration,
  CTC-06 resource accounting and CTC-14.06 recharge history.
  Preserve completion callbacks while suppressing the specified passive effects.
  Do not add the superseded two-use cooldown.

### 宇田川巴 — 豚骨酱油拉面大姐

Band: Afterglow. Color: `#BB0033`.

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。

[主动]：

（1）自己的移动阶段开始前，若自己不位于“银河拉面馆”：可使用1个[火罐]，依次进行以下操作：

1. 使此次移动的起点向[绝对距离]“银河拉面馆”更近的方向移动10格。
2. 若上述操作使起点向前移动并超过“银河拉面馆”，自己补充1个[火罐]。

（2）自己位于“银河拉面馆”时：可选择[停留]1回合，并获得一个[巴]状态。
[巴]状态：无法使用[主动]（2）效果，经过CiRCLE时移除。

#### Script allocation

- `AG04AutomaticAbility1`: [自动]（1）, four-turn recovery, initial amount/cap; references the original main skill, now [主动]（1）.
- `AG04ActiveAbility1`: [主动]（1）, origin adjustment, refund, and location restriction.
- `AG04ActiveAbility2`: [主动]（2）, location-gated optional stay, once per lap.

#### Implementation APIs

- [主动]（1）: validate the pre-movement phase and restaurant exclusion, spend
  one [火罐], select the ten-space origin adjustment using absolute distance,
  then apply the stated forward-past-restaurant refund. Do not substitute
  ordinary relocation without defining its event behavior.
- Available: `MatchBoardStateData.GetBoardDistance` with
  `BoardDistanceMode.AbsoluteNumber` (S05 / CTC-02.05).
- Missing: CTC-08.03 movement-origin replacement, CTC-06 resource spending and
  the explicitly authored refund, and CTC-14.06 recharge history. Cancellation
  alone does not create a resource refund.
- [主动]（2） building block: `StatusController.ServerModifyStatusStacks(
  playerId, statusId, stackChange)` with the authored [停留] status.
  The caller must enforce location and CTC-14.04 once-per-lap allowance.
  Do not assume one status stack equals one turn without verifying its lifecycle.

### 上原绯玛丽 — 大家一起迈出新的一步

Band: Afterglow. Color: `#FF9999`.

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。

[主动]：

（1）自己的移动阶段前：可使用1个[火罐]，选择此次移动仅在格子序号为单数的格子上进行，或仅在格子序号为双数的格子上进行。

#### Script allocation

- `AG05AutomaticAbility1`: [自动]（1）, Four-turn recovery, initial amount/cap; references the original main skill, now [主动]（1）.
- `AG05ActiveAbility1`: [主动]（1）, cost and odd/even traversal choice.

#### Implementation APIs

- Sequence: before the movement phase, validate/spend one [火罐], request the
  odd/even choice, and bind that traversal policy to this movement.
- Missing: CTC-03.05 option selection, CTC-08.08 traversal policy, CTC-06
  resource accounting and CTC-14.06 recharge history.
- `MovementPlanController.ServerSetMovementPlan(playerId, movementPlan)` can
  store a valid plan, but `MovementPlan` has no parity-policy field.
  A fixed-step plan or doubled step count is not equivalent to parity traversal.

### 羽泽鸫 — 伟大的平凡

Band: Afterglow. Color: `#FFEE88`.

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。

[主动]：

（1）自己进行[主要移动]时：可使用1个[火罐]，使本次移动反向。若如此做，在自己的下回合开始时进行1次双倍[投掷]的移动。

因该技能的效果反向移动[经过]CiRCLE时，不获得[CiRCLE奖励]。

#### Script allocation

- `AG06AutomaticAbility1`: [自动]（1）, Four-turn recovery, initial amount/cap; references the original main skill, now [主动]（1）.
- `AG06ActiveAbility1`: [主动]（1）, reversal, cost, delayed doubled roll/movement, and reward suppression.

#### Implementation APIs

- Sequence: validate active [主要移动], spend one [火罐], reverse its direction,
  and schedule the additional movement at this player's next turn start.
- Existing building blocks: `DiceController.ServerTryBeginRoll(parentEffect,
  actingPlayerId, actionType, getDiceTypes, onCompleted)` and the parent-effect
  `PlayerRelocationController.ServerTryMovePlayer` overload. These do not
  provide active-chain reversal or next-turn scheduling by themselves.
- Missing: CTC-08.04 active direction changes, CTC-14.01 scheduling,
  CTC-09.06 reward suppression, CTC-06 resources and CTC-14.06 recharge history.
- Preserve “双倍[投掷]” until its dice semantics are confirmed; do not silently
  replace it with doubling the final result. The later movement must complete
  its normal reaction/settlement flow under the applicable rules.

## Cards

### 宣战布告

[手]：

[反击][使用者]或[使用者]拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家，依次进行以下操作：

1. 被[指定]的玩家[支付][使用者]500资金。
2. [使用者]抽1张卡。

#### Script allocation

- `AG07HandEffect`: entire [手] reaction, including target, payment, and draw.

#### Implementation APIs

- Sequence: `PaymentController.ServerTryPlayerPayPlayer` (target pays user 500), then `DrawCardController.ServerTryBeginDrawCards` (one). Missing: a validated incoming card-effect context covering either the user or their property.

### Y.O.L.O

你的任意[投掷]结果结算前：打出此卡，并[投掷]1d4，使原[投掷]结果增加此次1d4的结果。

#### Script allocation

- `AG08PlayEffect`: entire unnumbered pre-resolution dice-increase reaction.

#### Implementation APIs

- Call: `DiceController.ServerTryBeginRoll` for the additional 1d4. Missing: an API to modify another pending roll's final result and a safe continuation while that roll is active; starting a second roll immediately is rejected.

### 商店街的青梅竹马

依次进行以下操作：

1. [投掷]1d6。
2. 根据此次[投掷]结果，[传送]到商店街自己拥有的对应格子（从商店街格子开始数）。若结果大于自己拥有的商店街格子数量，改为[传送]到商店街。

此次[传送]视为你的[主要移动]。

#### Script allocation

- `AG09PlayEffect`: entire unnumbered roll, destination selection/fallback,
  and teleport main movement.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (1d6) and `MovementPlanController.ServerSetMovementPlan` with `MovementPlan.CreateTeleport`. Missing: ordered owned-property selection and main-movement entry; settle the destination according to the applicable main-movement rules.

### 即使夕阳落山

[特]：

第100轮次开始前此卡的X为原本X的一半。

[手]：

[指定][使用者]拥有的四个有房子的格子和[使用者]以外的所有玩家。将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10。依次进行以下操作：

1. 删除所有被[指定]格子上的1栋房；

2. 所有被[指定]的玩家[支付][使用者]X资金。

#### Script allocation

- `AG10SpecialEffect`: [特], pre-round-100 adjustment of X;
  this modifies X rather than restricting play.
- `AG10HandEffect`: entire [手] block, including targets, valuation,
  house removal, and payment. Its numbered steps remain together.

#### Implementation APIs

- Call: `PaymentController.ServerTryPlayerPayPlayer` using the total required across eligible payers. Missing: four-property selection, authorized house removal/rent refresh, round-100 tracking, valuation/halving order, and zero-payer handling.

### [美竹兰]像往常一样

（1）此卡可作为[反击]使用。

（2）受到异常移动效果（包括你的技能）的回合结束前：回到起始地点，并取消所有受到的效果，不进行任何[结算]。

#### Script allocation

- `AG11Effect1`: （1）, reaction-use permission.
- `AG11Effect2`: entire （2）, return and the stated cancellation scope.
  The compensation/rollback semantics remain unresolved.

#### Implementation APIs

- No complete API: capture the relevant origin and abnormal-movement effects, then implement selective reversal and rollback. `PlayerRelocationController.ServerTryTeleportPlayer` alone cannot undo money, statuses, or triggered effects.

### [青叶摩卡]0.5倍速

（1）将此卡放置在场上，并获得3个[奇迹水晶]。你的回合结束时，移除1个[奇迹水晶]。[奇迹水晶]为0时，将此卡放入弃卡区。

（2）此卡在场时，用于确定你的移动步数的[投掷]的最终结果除以2，向上取整；你的所有资金[支付]与[消耗]减半。

#### Script allocation

- `AG12Effect1`: entire (1), placement, crystals, decay, and discard.
- `AG12Effect2`: entire (2), field-only movement and payment modifiers.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `PaymentController.ServerTryMultiplyMoneyAmount` (1/2) at matching stages. Missing: three-crystal lifetime and final movement-total halving; confirm money rounding against the payment API's rounding contract.

### 无论是何种颜色的夕阳

[投掷]1d6，根据结果获得对应效果。若结果严格大于6，从1开始重新计数。

- 结果为1：立刻获得1500资金。
- 结果为2：从弃卡区中选择1张卡，放置到抽卡区顶部。
- 结果为3：抽1张卡。
- 结果为4：选择补满[火罐]，或获得2000资金。
- 结果为5：从弃卡区中选择1张卡，加入手牌。
- 结果为6：选择结果1至5对应效果中的3个效果触发。

#### Script allocation

- `AG13PlayEffect`: entire unnumbered roll and result dispatch, including
  the three-effect choice on six and wraparound above six.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll`, `PaymentController.ServerTryPrintMoney`, `DrawCardController.ServerTryBeginDrawCards`, and `CardZoneController.ServerTryMoveCard` for selected discard retrieval/top insertion. Missing: private card/branch choices and fire-pot refill; direct retrieval is not automatically a draw.

### 回家的路上绕个道

（1）此卡可作为[反击]使用。

（2）依次进行以下操作：

1. 进行1次“Afterglow”式的移动，视为使用1次初始Afterglow角色的[主动]（1）效果。
2. 将用于确定下一次移动步数的[投掷]变更为1d6。

#### Script allocation

- `AG14Effect1`: (1), reaction-use permission.
- `AG14Effect2`: entire (2), copied Afterglow movement and next-roll replacement.

#### Implementation APIs

- Call: `MovementPlanController.ServerSetMovementPlan` with a 1d6 dice plan for the next applicable move. Missing: shared Afterglow skill dispatch, its resource/cooldown semantics, reaction timing, and preservation of other movement modifiers.

### ONE OF US

依次进行以下操作：

1. 将此卡放置在场上。
2. 选择场上的另一个Afterglow角色，或拥有商店街格子的角色。
3. [指定]双方各1块地契，必须优先[指定]商店街格子。

被[指定]格子的[结算]收益由双方平分，第三方不参与分钱。先在地契原主人方完成[结算]，之后直接增加被分享方的资金；这次资金增加不受其他任何效果影响。

其中一方破产时：将两张被[指定]的地契放置在此卡上，并转移到存活方的游戏区。存活方视为拥有这两张地契。

#### Script allocation

- `AG15PlayEffect`: placement and selection of the partner and linked deeds.
- `AG15FieldEffect`: linked-property revenue sharing, third-party exclusion,
  bankruptcy transfer, and resulting ownership while this arrangement applies.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and, where authorized, `PaymentController.ServerTryPrintMoneyWithoutEvents`. Missing: paired-deed revenue sharing, exact direct-credit semantics, bankruptcy interception, ownership transfer, and exclusion of third-party recipients.

### 朝同一片天空迈进

开局时抽到此卡，将此卡洗回。

[反击]抽出此卡时，立刻打出，并根据你的手牌数进行对应操作：

- 手牌数至少为3：获得手牌数×600的资金。
- 手牌数小于3：抽1张卡。

#### Script allocation

- `AG16SpecialEffect`: entire unnumbered draw-triggered use, opening-draw
  exception, and hand-count-dependent money/draw outcome.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney`, `DrawCardController.ServerTryBeginDrawCards`, `CardZoneController.ServerTryMoveCardToEnd`, and `DeckController.ServerShuffleDeck`. Missing: immediate on-draw play and initial-hand replacement handling; fix the hand-count sampling point.

### 绯红之魂

[手]：

依次进行以下操作：

1. 将此卡放置在[使用者]的[场地]。
2. 选择[消耗]1到5次500资金，并在此卡上放置对应数量的[奇迹水晶]。

[持续]：

（1）[反击][拥有者]因导致的[消耗]或[支付]时：可选择使用此卡的1个[奇迹水晶]，使此次[消耗]或[支付]金额减少1000，最少为0。若此次为[支付]，被[支付]玩家[获得]500资金。

（2）[拥有者]使用自己原有的[主动]（1）技能时：移除此卡的1个[奇迹水晶]。

（3）此卡上不再拥有[奇迹水晶]时：将此卡放入[使用者]弃卡区。

#### Script allocation

- `AG17HandEffect`: entire [手] block, placement and paid crystal initialization.
- `AG17FieldEffect1`: [持续]（1）, cost, reduction, and payee compensation.
- `AG17FieldEffect2`: [持续]（2）, crystal loss on use of the original [主动]（1） ability.
- `AG17FieldEffect3`: [持续]（3）, discard when crystals are exhausted.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, sequential `PaymentController.ServerTryDeleteMoney`, `ServerTryAddMoneyAmountDifference`, and `ServerTryPrintMoney`. Missing: crystal accounting, skill-use consumption, zero-floor enforcement, and clarification of the incomplete reaction condition.

### 刻入天穹傲岸的烈光

[反击]当你[经过]一名角色时：你可以打出此卡，依次进行以下操作：

1. 对方[支付]你等于对方最贵格子基础购买价格一半的资金。
2. 你[支付]对方等于你最贵格子基础购买价格一半的资金。

#### Script allocation

- `AG18PlayEffect`: entire pass reaction and both ordered property-value payments.

#### Implementation APIs

- Call: `PaymentController.ServerTryPlayerPayPlayer` twice in the stated order. Determine each player's highest authored property price and halve it; define empty-portfolio handling and whether both valuations are captured before the first payment.

### [宇田川巴]商店街的救世主

[反击]符合下列任一时机时，你可以打出此卡，进行该时机对应的操作：

- 其他玩家抵押商店街地契时：你立刻支付常规收购价一半的资金，从该玩家处收购该地契。
- 你抵押商店街地契时：你额外获得1份抵押收益，并将该地契翻回。

#### Script allocation

- `AG19PlayEffect`: entire unnumbered mortgage reaction, with separate
  conditional branches for another player's mortgage and the user's mortgage.

#### Implementation APIs

- Related calls: `TileController.TryGetMortgagedPropertyAcquisitionQuote` and `PaymentController.ServerTryPrintMoney`. Missing: discounted acquisition and free redemption. The normal acquisition API requires its exact quote and retains the mortgage; it cannot implement these overrides unchanged.

### [上原绯玛丽]如果并非没问题

[反击]你的一次[投掷]结果小于6时：你可以打出此卡，使此次[投掷]结果增加1。

#### Script allocation

- `AG20PlayEffect`: entire low-roll reaction and result increase.

#### Implementation APIs

- No verified final-roll mutation API exists. Add a reaction-safe +1 adjustment to the qualifying pending roll; `MovementPlanController.ServerSetDiceTotal` changes a stored movement plan, not arbitrary dice results.

### [羽泽鸫]微小的『能做到』的事

打出此卡时，选择以下一个效果发动：

（1）[反击]时机合适时：使任意结果只有数字区间的效果以理论最大值或最小值结算。除@Tsugu ycm以外，无法直接改变骰子点数。例如：可以对“Y.O.L.O”生效，但无法对“无论是何种颜色的夕阳”生效。

（2）视为打出1张@Tsugu ycm。

#### Script allocation

- `AG21PlayEffect`: shared play/choice and the selected （1） or （2）
  alternative. These are explicitly choices, not independent activations.
  Preserve the timing and exclusions of each branch.

#### Implementation APIs

- No complete API: implement eligible numeric-range substitution and reusable @Tsugu ycm effect dispatch. Preserve the stated dice exclusions and normal targeting/cost/reaction checks; copying display text is not effect execution.
