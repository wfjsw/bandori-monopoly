# Roselia

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
RO-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 对音乐的纯粹

[自动]：

（1）自己在本局游戏购买的第一个非Live House格子的购买价格减半。在自己拥有任意初始Live House格子前，该格子对自己视为Live House。

（2）自己购买任何Live House格子的地契时：购买价格减半。

#### Script allocation

- `RO01AutomaticBandEffect1`: [自动]（1）, first-purchase discount and conditional Live House trait.
- `RO01AutomaticBandEffect2`: [自动]（2）, Live House purchase discount.

#### Implementation APIs

- Related call: `PaymentController.ServerTryMultiplyMoneyAmount` for an authorized purchase discount. Missing: first-purchase history and player-specific Live House classification; `TileController.ServerSetTileType` changes global type and is not a per-player override.

## Characters

### 凑友希那 — 来练习吧

[资源]：

（1）[火罐]：初始0，上限1。

[自动]：

（1）自己初始获得“RiNG 4”格子，并从“RiNG 4”开始游戏。自己首次[经过]CiRCLE时，不获得[CiRCLE奖励]。

（2）自己的[移动终点]为任意RiNG时：获得1个[火罐]。自己不位于RiNG时，失去所有[火罐]。

[主动]：

（1）自己移动中[经过]首个任意RiNG格子时：可选择使自己停止移动。

（2）其他玩家移动[经过]自己时：可使用1个[火罐]，令那名玩家[强制停下]并[结算]。

#### Script allocation

- `RO02AutomaticAbility1`: [自动]（1）, initial property/location and first-pass exception.
- `RO02AutomaticAbility2`: [自动]（2）, arrival fire pots, cap, and loss away from RiNG.
- `RO02ActiveAbility1`: [主动]（1）, optional stop at the first passed RiNG.
- `RO02ActiveAbility2`: [主动]（2）, passing-player interception, cost, stop, and settlement.

#### Implementation APIs

- Call: `TileController.ServerTryAddPropertyOwner` during setup. Missing: custom starting position, first-CiRCLE exclusion, fire-pot lifecycle, and voluntary/other-player force-stop with settlement; normal teleport would emit setup-inappropriate relocation events.

### 白金燐子 — 即使1cm也要前进

[资源]：

（1）[火罐]：初始3，上限3。

[自动]：

（1）自己每次[经过]CiRCLE时：获得3个[火罐]。

[主动]：

（1）使用X个[火罐]，获得一层[霸体]状态，并立刻进入移动阶段，进行固定为X×6格的移动。此技能在正常状态或[眩晕]状态下均可使用。

[霸体]状态：不受[异常移动效果]影响，回合结束时移除该状态。

#### Script allocation

- `RO03AutomaticAbility1`: [自动]（1）, CiRCLE fire pots and initial amount/cap.
- `RO03ActiveAbility1`: [主动]（1）, resource choice, fixed result, stun-use exception, and movement protection.

#### Implementation APIs

- Related call: `MovementPlanController.ServerSetDiceTotal` after the normal dice pipeline has supplied its result. Missing: roll-result replacement timing, fire-pot accounting, activation while stunned, and abnormal-movement immunity; a fixed-step plan would bypass dice semantics.

### 宇田川亚子 — 对帅气的憧憬

[自动]：

（1）自己[经过]CiRCLE领取[CiRCLE奖励]时，如果自己的场上存在背面朝上的卡：此次领取奖励改为将自己场上背面朝上的卡加入自己手牌。

（2）自己场上背面朝上的卡为[反击]卡时，可在符合条件时直接打出该卡。

[主动]：

（1）自己[经过]CiRCLE并选择[抽卡]奖励时：可依次进行以下操作：

1. 观看抽卡区顶部至多2张卡。
2. 选择其中1张，加入自己手牌。
3. 将剩余的卡放入弃卡区，或花费500资金，将其背面朝上放置在自己的场上。

#### Script allocation

- `RO04AutomaticAbility1`: [自动]（1）, mandatory reward replacement retrieving face-down field cards.
- `RO04AutomaticAbility2`: [自动]（2）, standing permission to play eligible face-down reaction cards; not another card play.
- `RO04ActiveAbility1`: [主动]（1）, optional reward inspection, selection, and discard/paid-placement choice.
- The original cost wording does not identify a payment recipient; this pass does not choose a money-operation type.

#### Implementation APIs

- Building blocks: `CardZoneController.ServerTryMoveCard` and `PaymentController.ServerTryDeleteMoney`. Missing: private top-two choice, face-down field storage, CiRCLE reward replacement, and authenticated counter-card play from that zone.

### 冰川纱夜 — 踏上荆棘之路的觉悟

[资源]：

（1）[火罐]：初始10，上限10。

[自动]：

（1）自己每次[经过]CiRCLE时：获得2个[火罐]。自己每次被其他玩家[经过]时，获得1个[火罐]。

（2）第一个玩家被淘汰后：自己[经过]CiRCLE时获得的[火罐]数量增加1。

[主动]：

（1）自己进行确定[主要移动]步数的[投掷]后：可使用6个[火罐]，将本次非[传送]的[主要移动]增加1格或2格。每回合限1次。

#### Script allocation

- `RO05AutomaticAbility1`: [自动]（1）, both fire-pot triggers and initial amount/cap.
- `RO05AutomaticAbility2`: [自动]（2）, first-elimination reward increase.
- `RO05ActiveAbility1`: [主动]（1）, cost, chosen movement extension, and usage limit.

#### Implementation APIs

- No complete modifier API: author fire-pot recharge/costs, once-per-turn activation, first-elimination tracking, and +1/+2 to non-teleport main movement. Stored-plan setters alone do not establish the correct result-modification timing.

### 今井莉莎 — 慈爱女神

[资源]：

（1）[火罐]：初始2，上限2。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

（2）自己提供的每个临时[火罐]，在提供后自己的第二次回合开始前失去。其他玩家使用自己提供的临时[火罐]时，需[支付]自己600资金；被“冲榜”类效果被动消耗时无需[支付]。

[主动]：

（1）自己的移动阶段前：可使用1个[火罐]，将此次移动改为[传送]到与自己[正向距离]最近的其他角色所在格子并[结算]（包括同格的其他角色；并列最近时由自己选择。同格[传送]不触发[经过]）。

此次[传送]的[结算]前，自己可选择依次进行以下操作：

- 额外使用1个[火罐]。
- 使与自己位于同一格子的另一名角色获得1个可超过上限的临时[火罐]。

若如此做，此次[传送]不触发任何[结算]。

#### Script allocation

- `RO06AutomaticAbility1`: [自动]（1）, CiRCLE fire pots and initial amount/cap.
- `RO06AutomaticAbility2`: [自动]（2）, temporary-resource expiry and usage payment; preserves the passive-consumption exception.
- `RO06ActiveAbility1`: [主动]（1）, teleport, settlement, and embedded optional resource/settlement replacement.
- RES-01: expire each grant at Lisa's second BeforeTurnStart after the grant,
  not at the recipient's turn. Preserve grant identities and explicit spending
  choice. Consume selected temporary fire pots before payment to Lisa, so nested
  payment reactions cannot spend them again. Preserve the passive-use exception.
  Payment cancellation, zero/partial payment, failure, or bankruptcy does not
  block this already-admitted continuation unless explicitly canceled.
- Character entry is zero with cap two; the written initial two fire pots are
  granted by the game-start effect, not by character replacement/re-entry.

#### Implementation APIs

- Calls: `MovementPlanController.ServerSetMovementPlan` with a teleport plan, `ServerSetResolveTileAfterMovement`, and `PaymentController.ServerTryPlayerPayPlayer` for temporary-token use. Missing: nearest-player choice, temporary over-cap fire pots, expiry/provenance, and settlement-time suppression.
- S05 `MatchBoardStateData.GetNearestPlayersSnapshot` with
  `BoardDistanceMode.Forward` includes same-tile candidates and retains ties.
  Supply other eligible players; the helper does not choose a tied player.
  API-01 same-tile teleport skips passing and retains landing. Resource and
  settlement-time integration above remain Pending.

## Cards

### [凑友希那]louder

每次打出此卡时：使RiNG的价格基础乘数增加5。

#### Script allocation

- `RO07PlayEffect`: entire unnumbered RiNG base-multiplier increase.

#### Implementation APIs

- Calls: `TileController.TryGetRingRentMultiplier` then `ServerSetRingRentMultiplier` with the validated value plus five. Confirm that the text's base multiplier means this shared runtime rent multiplier.

### 学生会的检查

移动结束后，若前后3格内存在你拥有地契的格子：可在[结算]前打出此卡，向抽卡区加入1张“觉悟”。本回合无法加盖房屋，但可支付该格子1层房屋建造价格的一半，将此卡放在该格子上。

下一个[经过]该格子、且[移动终点]不在该格子的其他玩家，在该格子[强制停下]并进行[结算]。因此卡[强制停下]的玩家，此次[结算]地租价格为原本的一半。

此卡进入弃卡区时，若[结算]获得的资金小于本卡原应拿到的资金，或未进行[结算]：获得发动此卡消耗的资金。

#### Script allocation

- `RO08PlayEffect`: play eligibility, 觉悟 creation, turn construction restriction,
  optional payment, and placement on the selected property.
- `RO08FieldEffect`: passing-player interception, correlated half rent, and
  the stated discard-time refund check; retain the activation's recorded cost.

#### Implementation APIs

- Calls: `CardCreationController.ServerCreateCard`, `CardZoneController.ServerTryMoveCardToEnd`, and payment delete/multiply/print operations. Missing: force-stop, construction prohibition, card-scoped cost/rent accounting, refund criteria, and source-correlated half-rent handling.

### 向着顶点

移动到下一个可被购买的Live House格子，视为你的[主要移动]。

若所有Live House格子均已被购买，可花费1.5倍价格，为属于你的1个Live House格子加盖1层房屋。

#### Script allocation

- `RO09PlayEffect`: entire movement or alternative paid-construction resolution.

#### Implementation APIs

- Related calls: `MovementPlanController.ServerSetMovementPlan` and `TileController.ServerTryResolvePropertyDevelopment`. Missing: next-purchasable Live House traversal and a validated 1.5× construction-cost override; the normal development API requires its exact quote.

### 曲奇时间

依次进行以下操作：

1. 将自己弃卡区的全部卡返回抽卡区，并洗切。
2. 获得500×X资金；X为上述操作返回的卡的总数。

#### Script allocation

- `RO10PlayEffect`: entire recycle/shuffle and returned-card-count money reward.

#### Implementation APIs

- Sequence: snapshot discard count, `DeckController.ServerRecycleDiscardIntoDeck`, then `PaymentController.ServerTryPrintMoney` for 500 × successfully returned count. Reconcile partial recycle failure before rewarding or retrying.

### Fire bird

支付1600资金，将此卡放置在自己的场地上，并为此卡添加X个[奇迹水晶]；X由自己选择。

自己的场地上存在此卡时：

- 每回合结束时：失去400资金，并移除此卡的1个[奇迹水晶]。
- 自己的所有格子收费变为原本的1.5倍。

此卡的[奇迹水晶]耗尽时，将此卡放入弃卡区。

#### Script allocation

- `RO11PlayEffect`: payment, field placement, and chosen crystal initialization.
- `RO11FieldEffect`: turn-end loss/decay, rent multiplier, and exhaustion discard.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney`, `CardZoneController.ServerTryMoveCardToEnd`, and `PaymentController.ServerTryMultiplyMoneyAmount` (3/2) for eligible rent. Missing: chosen crystal quantity, turn-end decay, and clarification of the initial payment's recipient.

### NFO

[投掷]1d6，并在回合结束后获得1层[停留]。根据此次[投掷]结果进行对应操作；若结果严格大于6，从1开始重新计数。

- 结果为1：获得1500资金。
- 结果为2：立刻移动到你前方最近1名玩家的前方1格，并使其格子上的玩家[分摊][支付]你1000资金。此次移动视为本回合的[主要移动]，但不进行[结算]。
- 结果为3：将此卡置于场上。你下次付款时，自动减免1000资金的消耗，并将此卡置入弃卡区。
- 结果为4：选择你弃卡区中1张满足打出条件的卡，打出该卡。
- 结果为5：为自己的角色卡添加6个[奇迹水晶]。你每次获得资金时，可使用1个[奇迹水晶]，使此次金额增加300。
- 结果为6：依次获得结果1至5的全部效果。

#### Script allocation

- `RO12PlayEffect`: roll, delayed stay, and result dispatch, including result six.
- `RO12FieldEffect`: result-three field card's next-payment reduction and discard.
- Result five grants character-associated crystal/spending behavior; its
  lifetime must not depend on the card remaining active. Exact attachment
  ownership remains an implementation requirement.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll`, `StatusController.ServerModifyStatusStacks`, movement-plan setters, card-zone movement, and payment print/transfer/additive operations. Missing: branch dispatch, delayed stay, discard-card play, character crystals, and ordered execution of results 1–5.

### live前的准备

[反击][经过]“江户川乐器店”时：可打出此卡，使自己在“江户川乐器店”[强制停下]，并进行[结算]。

#### Script allocation

- `RO13PlayEffect`: entire instrument-shop pass reaction, stop, and settlement.

#### Implementation APIs

- No complete force-stop API: react to passing the instrument shop, terminate the remaining movement, and request exactly one settlement. `TileResolutionController.ServerTryBeginResolution` supplies settlement only, not movement cancellation.

### 选择自己的舞台

[反击]受到[除外]以外的[异常移动效果]影响时：可打出此卡，选择自己的本次移动是否触发[结算]；若无法移动，则选择在该回合结束时是否触发[结算]。

#### Script allocation

- `RO14PlayEffect`: entire abnormal-movement reaction and settlement choice.

#### Implementation APIs

- Related call: `MovementPlanController.ServerSetResolveTileAfterMovement`. Missing: abnormal-movement classification excluding banishment, alteration of already active movement, and deferred end-turn settlement when unable to move.

### [冰川纱夜]弹奏弹奏弹奏，继续弹奏

[反击]时机合适时打出此卡，视为使用1次此卡[使用者]的技能。

#### Script allocation

- `RO15PlayEffect`: entire reaction invoking the user's eligible ability.
  The invoked ability retains its own effect script.

#### Implementation APIs

- No complete ability-invocation API: dispatch the user's selected character skill with its timing, targets, costs, and usage limits. Resolve whether this card waives any normal cost or limit before implementation.

### [宇田川亚子]黑暗大魔姬亚子

（1）[反击]你即将向其他玩家[支付]资金时：可打出此卡，使自己获得1层[眩晕]。若此卡从手牌以外的地方打出，你抽1张卡。

#### Script allocation

- `RO16Effect1`: entire （1）, payment reaction, stun, and source-zone-dependent draw.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks` for authored stun and `DrawCardController.ServerTryBeginDrawCards` when played outside Hand. Missing: authorized non-hand play and captured source-zone provenance; stun does not itself cancel the pending payment.

### [白金燐子]Ringing Bloom

（1）将此卡放置于自身场上。

（2）你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半。

（3）你的任意非RiNG格子收费后，依次进行以下操作：

1. 将此卡置入弃卡区。
2. 你获得500×X资金；X为你收费格子上的房屋数。

#### Script allocation

- `RO17Effect1`: （1）, field placement.
- `RO17Effect2`: entire （2）, effective house counts and associated rent reduction.
- `RO17Effect3`: entire （3）, non-RiNG charge response, discard, and money.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryMultiplyMoneyAmount`, and `ServerTryPrintMoney`. Missing: virtual house-count queries, source-scoped rent overrides, and timing for sampling the reward's house count; do not physically add houses.

### 蓝玫瑰的骄傲

打出此卡时，根据你拥有的Live House格子数量与其他格子数量，进行对应操作：

（1）若Live House格子数量大于等于其他格子数量：获得等于这些Live House格子地契价值20%的资金。

（2）若Live House格子数量少于其他格子数量：[传送]至下一个未被购买的Live House格子。若已没有未被购买的Live House格子，改为[传送]至“RiNG 4”，不触发[结算]。两种情况均视为你的[主要移动]。

#### Script allocation

- `RO18Effect1`: （1）, qualifying Live House ownership and valuation reward.
- `RO18Effect2`: entire （2）, complementary ownership condition and teleport/fallback.
- Evaluate both against the same play context; their eligibility is mutually
  exclusive, not two independent card plays.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney` or `MovementPlanController.ServerSetMovementPlan` with a teleport plan. Missing: Live House portfolio valuation, destination selection, main-movement entry, and explicit settlement policy for the unowned-property branch.

### Sprechchor

在Live House格子开始回合时：可打出此卡，依次进行以下操作：

1. [投掷]1d20。
2. 获得1000+X×120资金；X为此次[投掷]结果。

#### Script allocation

- `RO19PlayEffect`: entire location/timing eligibility, roll, and money formula.

#### Implementation APIs

- Sequence: validate the turn-start Live House condition, `DiceController.ServerTryBeginRoll` (1d20), then `PaymentController.ServerTryPrintMoney` for 1000 + result × 120.

### 轨迹

[反击][特]

（1）场上有玩家[破产]时，依次进行以下操作：

1. 展示此卡。
2. 获得那名玩家的任意1张地契，自动免费赎回。
3. 拆除该地契对应格子上的所有房屋。

#### Script allocation

- `RO20SpecialEffect1`: entire [特]（1） bankruptcy reaction, reveal,
  deed acquisition/redemption, and demolition.

#### Implementation APIs

- Related call: `TileController.ServerTryAddPropertyOwner`. Missing: coordinated bankruptcy-asset interception, free redemption, house demolition/rent refresh, and public card reveal. Adding ownership alone does not perform the required transfer or cleanup.

### [今井莉莎]必然的联系

[反击]你使用技能进行[传送]后：可打出此卡，[指定]1个与你位于同一格子的角色，并将此卡放在对方的游戏区。

你对那名角色使用技能时，可以无视按[正向距离]选择最近角色的限制。你发动该效果后，将此卡放入弃卡区。

#### Script allocation

- `RO21PlayEffect`: post-ability-teleport reaction, co-located target, and placement.
- `RO21FieldEffect`: target-specific nearest-player exception and discard on use.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for another player's Field and later Discard. Missing: skill-teleport provenance, same-tile target choice, and a source-scoped nearest-target exception consumed after successful use.
- S05 `MatchBoardStateData.GetPlayersAtTileSnapshot` supplies same-tile candidates;
  authenticated target choice and the one-use nearest-target exception remain Pending.

### [衍生] 觉悟

[特]：此卡[移除]时获得1000资金。

[手]：[移除]此卡。

#### Script allocation

- `RO22SpecialEffect`: [特], reward on removal.
- `RO22HandEffect`: [手], self-removal. Do not also grant the special reward here.

#### Implementation APIs

- Calls: `CardRemovalController.ServerRemoveCard` and `PaymentController.ServerTryPrintMoney` (1000). Missing: a removal-trigger path that covers every removal source exactly once, not only this card's hand effect.
