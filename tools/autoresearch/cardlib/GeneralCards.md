# General Cards

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
GC-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Cards

### @Tsugu ycm

[手]：

投掷3d10并根据结果依次进行以下操作：

1. 结果至少为22则抽1张卡；

2. 结果至少为26则进入移动阶段并将本回合的[主要移动]改为[传送]到“Bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；

3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；

4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“Bandori车站”并[结算]。

#### Script allocation

- `GC01HandEffect`: entire [手] block, including the roll and all four
  threshold-dependent steps. The numbered steps are not separate effects.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (nested, GeneralDice, 3d10); `DrawCardController.ServerTryBeginDrawCards` (one card); `PaymentController.ServerTryPrintMoney` (1000 to the user). Evaluate thresholds in order and await each nested operation.
- Movement: `MovementPlanController.ServerSetMovementPlan` with `MovementPlan.CreateTeleport`, then `ServerSetResolveTileAfterMovement`. These only configure movement. Missing integration: effect-driven movement-phase entry, optional remote purchase, and a turn-scoped purchase discount with a zero floor.

### 登上武道馆

[手]：

[使用者]以外的所有[存活]玩家[分摊]向[使用者][支付]2000资金；无法付款的玩家（包括处于[除外]状态的玩家）仍计入分摊，其付款条目在分摊后取消，其他玩家的份额不变。若没有其他[存活]玩家，则[使用者][获得]2000资金。

#### Script allocation

- `GC02HandEffect`: entire [手] block, including participant selection,
  ordinary payment splitting and the zero-other-player money gain.

#### Implementation APIs

- Proposed API — implementation pending: `PaymentController.ServerTryChangeMoney`,
  with the resolving effect as parent, all other alive players as payers,
  the user as sole payee, and base total 2000.
- Select participants at step begin. Do not prefilter unavailable or [除外]
  payers; cancel their entries after splitting without redistribution.
  The pipeline must revalidate payer/payee availability before each transaction.
- Use ordinary modifiers, splitting and rounding. Do not precompute rounded
  shares or submit X times payer count; this is not event-free money.
- An initially empty payer list invokes PrintMoney. Canceling all entries
  of a nonempty payer list does not change the operation into PrintMoney.
  The current `ServerTryPlayerPayPlayer` API does not support empty payers.
- Await completion; admission alone does not confirm payment.

#### Accepted population and verification — DEC-01 / DEC-02

- Alive means not bankrupt and not departed; disconnection alone is not
  departure. This population-based operation does not declare [指定] targets
  or relax other effects' restrictions on targeting [除外] players.
- Acceptance cases: three payers and no modifiers produce shares of 670.
  A pre-split +100 adjustment produces shares of 700, not 710.
- Verify unavailable players remain in the divisor, canceled entries do not
  redistribute shares, and availability changes are checked before settlement.
- Verify an initially empty payer list produces ordinary PrintMoney with
  base total 2000, but cancellation of every initially supplied payer does not.
- S04 / CTC-01.01 supplies `MatchPlayerStateData.GetOtherAlivePlayerIdsSnapshot`.
  Do not use an OnBoard-only query to filter this population-based operation.
  CTC-10.08 outcome reporting remains Pending when later behavior needs actual
  settlement information. Compilation/runtime checks: Not Run.

### GREAT

[手]：

依次进行以下操作：

1. [移除]此卡；

2. 将一张“PERFECT”加入抽卡区并洗切；

3. [获得]2000资金。

#### Script allocation

- `GC03HandEffect`: entire [手] block; removal, creation/shuffle, and
  money are ordered steps of this one effect.

#### Implementation APIs

- Calls, in order: `CardRemovalController.ServerRemoveCard`; `CardCreationController.ServerCreateCard` (PERFECT definition, Deck); `DeckController.ServerShuffleDeck`; `PaymentController.ServerTryPrintMoney` (2000 to the user, nested).
- Requirements: use runtime identity and the actual source zone for removal, a registered card definition and valid insertion index for creation, and explicit failure handling between steps. These calls do not form an atomic transaction.

### [衍生]PERFECT

[手]：

依次进行以下操作：

1. [移除]此卡；

2. 将一张“FEVER!”加入弃卡区；

3. [获得]3000资金。

#### Script allocation

- `GC04HandEffect`: entire [手] block; self-removal, FEVER! creation,
  and money remain in one resolution.

#### Implementation APIs

- Calls, in order: `CardRemovalController.ServerRemoveCard`; `CardCreationController.ServerCreateCard` (FEVER! definition, Discard); `PaymentController.ServerTryPrintMoney` (3000 to the user, nested).
- Requirements: validate runtime identity, source zone, registered definition, and insertion index. Handle partial failure explicitly; await the money operation before completing the effect.

### [衍生]FEVER!

[手]：

将此卡放置在[使用者]的[场地]。

[持续]：

（1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。

（2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。

#### Script allocation

- `GC05HandEffect`: [手] field placement.
- `GC05FieldEffect1`: [持续]（1）, including the field-count formula.
- `GC05FieldEffect2`: [持续]（2）, turn-start discard.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` to the user's Field; `PaymentController.ServerTryAddMoneyAmountDifference` during the matching additive money stage; `ServerTryMoveCardToEnd` from Field to Discard at the owner's turn start.
- Requirements: author a field-only responder, recompute X from the owner's field cards, and filter the relevant receipt operation/pair. Supply its operation ID and pair IDs when required. Clarify owner versus user if ownership can change.

### 10次招募（1回限定）

[手]：

[消耗]1500资金，抽1张卡。

#### Script allocation

- `GC06HandEffect`: entire [手] block, including payment and subsequent draw.
  The title does not supply an additional usage-limit effect.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney` (1500 from the user), followed by `DrawCardController.ServerTryBeginDrawCards` (one card); both use the resolving effect as parent.
- Requirements: define the cost-success condition before drawing. A completed money pipeline does not prove the full cost was paid. Authenticated card-use/cost orchestration remains incomplete; the title alone does not define a usage-limit rule.

### 该清CP了

（1）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物将在相邻的没有[CP点]的格子添加1个[CP点]。

[手]：

在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场地]添加6个[CP点]。在有[CP点]的格子[结算]时，使用该格1个[CP点]及自己[场地]上1个[CP点]，[获得]800资金。任一处不足1个时，不使用任一处资源，也不取得该笔资金。

#### Script allocation

- `GC07Effect1`: （1）, source-linked CP propagation at the next two turn starts.
- `GC07HandEffect`: entire [手] block, including CP placement and registration
  of its later settlement consumption/reward behavior.
- CP state and delayed behavior must survive card-zone changes as required
  by the rule; no separate field-card effect is stated. The missing CP
  removal quantity remains unresolved.

#### Implementation APIs

- Available calls: `StatusController.ServerModifyStatusStacks` for player CP only if CP is authored as a registered player status; `PaymentController.ServerTryPrintMoney` for the 800 reward.
- Missing support/design: tile CP status operations, legal tile selection, source-linked propagation for two turns, and settlement consumption. The text also omits the number of tile CP points removed; do not infer that quantity.

### 雨啊，快点来吧

[手]：

投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。

#### Script allocation

- `GC08HandEffect`: entire [手] block—roll, select targets including the
  user, then grant stay stacks. Status behavior belongs to its status source.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (nested, GeneralDice, 2d2); `EffectReactionController.RequestPlayerTargetSelection`; `StatusController.ServerModifyStatusStacks` (+1 using the authored stay-status ID).
- Requirements: select exactly X legal players including the user, then apply the status. Resolve insufficient eligible players and duplicate-target policy; author/verify the stay status's gameplay behavior separately from stack storage.

### 网络链接异常

[手]：

[反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行以下操作：

1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；

2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；

3. 事件卡的效果手牌则则抵消其所有的效果。

#### Script allocation

- `GC09HandEffect`: entire [手] reaction, including all three conditional
  cancellation branches; not one script per branch.

#### Implementation APIs

- Integration point: an authored reaction effect using `BaseResolvableEffect.CanRespondToEvent` and `Resolve`, once the required card/event pre-effect window and payload are available.
- Missing support: a verified API to cancel one pending target or invalidate the entire pending card/event effect. `CompleteCurrentResolution` completes the current responder; it is not cancellation of the intercepted effect.

### 尽力后的收获

[手]：

立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。

#### Script allocation

- `GC10HandEffect`: entire [手] block—integer choice, movement entry,
  main-movement replacement, and its settlement.

#### Implementation APIs

- Calls: `MovementPlanController.ServerSetMovementPlan` with `MovementPlan.CreateFixedStep(chosenSteps)`; the created plan enables destination settlement.
- Missing integration: an authenticated integer choice constrained to 1–6 and effect-driven movement-phase entry. Setting a plan does not start movement; do not substitute an immediate relocation for the turn's main movement.

### CiRCLE THANKS PARTY!

[手]：所有其他玩家可选择[消耗]500资金，你消耗500资金，将X设为因此卡[消耗]资金的玩家数量加1。根据X进行以下操作之一：

1. X至少为2则[使用者]投掷Xd20，如果结果至少为35则[使用者][获得]3000资金且其他因此卡[消耗]资金的玩家[获得]1500资金；

2. X等于1则[使用者]的本回合结束后获得一个额外回合。

#### Script allocation

- `GC11HandEffect`: entire [手] block—contributions, X calculation, and
  the selected dice/reward or extra-turn branch.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney` for each participant's 500; `DiceController.ServerTryBeginRoll` (nested, GeneralDice, Xd20); `PaymentController.ServerTryPrintMoney` for rewards. Its amount is a shared total: use 1500 × recipient count when rewarding multiple other participants together.
- Missing support/design: optional contribution prompts, reliable paid-participant accounting, and extra-turn scheduling. Clarify whether X counts the user before adding 1. Sequence nested operations; completion alone does not establish contribution.

### 安可

[手]：

[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。

#### Script allocation

- `GC12HandEffect`: entire [手] reaction, canceling the intercepted
  abnormal-movement effect and its defined dependent effects.

#### Implementation APIs

- Integration point: an authored reaction effect using `BaseResolvableEffect.CanRespondToEvent` and `Resolve`, with an authoritative abnormal-movement pre-effect context.
- Missing support/design: abnormal-movement classification and an API that invalidates the intercepted effect and its dependent effects. Neither completing the responder nor resetting a future movement plan provides this cancellation.
