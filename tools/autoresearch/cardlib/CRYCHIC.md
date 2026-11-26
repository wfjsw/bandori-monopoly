# CRYCHIC

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
CR-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 美好的往日幻影

[资源]：

（1）此卡的[奇迹水晶]：上限10。

[自动]：

（1）自己的手牌数没有上限。任何时刻，自己的手牌数至少为6时，获得[黑刀之夜]状态。无法获得或失去资金，无法从手中打出任何卡，且领取[CiRCLE奖励]时必须选择[抽卡]1张。

（2）自己的回合结束时，若自己的抽卡区与弃卡区中均没有卡：依次进行以下操作：

1. [移除]自己的[黑刀之夜]状态；

2. [移除]此卡与自己所有区域的全部“CRYCHIC”卡；

3. 将自己剩余的所有手牌放入抽卡区，并加入自己选择的、角色对应的“MyGO”或“Ave Mujica”卡，直至抽卡区共有10张卡，然后洗切；

4. 获得角色对应的“MyGO”或“Ave Mujica”乐队技能卡，然后自己[抽卡]2张。

（3）自己的手牌数每次从4增加至5时：为此卡添加1个[奇迹水晶]。此卡[移除]时，自己[获得]X×500资金；X为此卡上的[奇迹水晶]数量。

[黑刀之夜]状态：
1. 无法获得或失去资金；
2. 无法主动进行会直接[消耗]或[支付]资金的动作；
3. 无法从手中打出任何卡；
4. 领取[CiRCLE奖励]时必须选择[抽卡]1张；
5. 手牌数量从大等于6变成小等于5时减少1层。

#### Script allocation

- `CR01AutomaticBandEffect1`: [自动]（1）, hand limit, high-hand-count restrictions, and mandatory reward choice.
- `CR01AutomaticBandEffect2`: [自动]（2）, empty-pile transition, removals, selected replacement deck/band, and draw.
- `CR01AutomaticBandEffect3`: [自动]（3）, hand-count crystals and removal reward.
- The removal reward uses the source's retained crystal snapshot; effect 2 must not duplicate effect 3's reward. Generic money gain/loss restrictions are not narrowed to bank operations.

#### Implementation APIs

- Building blocks: card removal/creation/zone APIs, `DeckController.ServerShuffleDeck`, `DrawCardController.ServerTryBeginDrawCards`, and payment print/cancellation APIs. Missing: unlimited-hand rules, six-card restrictions, band replacement, ten-card selection, crystals, and coordinated transformation sequencing.

## Characters

### 高松灯（CRYCHIC） — 跌跌撞撞...

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

（2）用于确定自己移动步数的[投掷]结果小于等于6时：立即获得1层[停留]，并在回合结束时[结算]。

[主动]：

（1）自己的回合内任意时刻：可使用1个[火罐]，移除自己1层[停留]。

#### Script allocation

- `CR02AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `CR02AutomaticAbility2`: [自动]（2）, low-roll stay and delayed settlement.
- `CR02ActiveAbility1`: [主动]（1）, own-turn resource spending to remove stay.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks` and delayed `TileResolutionController.ServerTryBeginResolution`. Missing: fire-pot recharge/cost, own-turn activation, movement-roll threshold detection, and turn-end settlement scheduling.

### 椎名立希（CRYCHIC） — 克服劣等感

[主动]：

（1）用于确定自己移动步数的[投掷]结果，严格小于其他玩家中上一次用于确定移动步数的[投掷]结果，且那次结果不大于20、自己本回合未使用技能时，获得一层[劣等感]状态

[劣等感]状态：在进行确定自己移动步数的[投掷]后可重新[投掷]1次，并使用第二次的结果，然后移除该状态。

#### Script allocation

- `CR03ActiveAbility1`: [主动]（1）, history/usage eligibility and next-turn optional reroll.
- Eligibility must retain the qualifying roll/usage history; the original wording does not specify when the future choice is declared.

#### Implementation APIs

- Related call: `DiceController.ServerTryBeginRoll`. Missing: other-player last-roll history, this-turn skill-use tracking, next-turn entitlement, and replacing the first movement roll with the second result without resolving both.

### 丰川祥子（CRYCHIC） — 你愿意和我组建乐队吗？

[资源]：

（1）[火罐]：初始1，上限2。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

（2）其他玩家的[主要移动]结束时、[结算]前，且那名玩家位于自己前后5格内：那名玩家可选择[支付]自己300资金，使其[移动终点]沿最短路径向自己靠近1格。

（3）自己的乐队技能为“Ave Mujica”时：自己与其他玩家的距离在5格以内则处于[人偶模式]，但只有洗牌时才能为自己的乐队技能卡添加[奇迹水晶]。

[主动]：

（1）自己的[主要移动]结束时、[结算]前，若自己前后5格内有其他玩家：可使用1个[火罐]，选择其中1名玩家，使自己的[移动终点]沿最短路径向其靠近1格。

#### Script allocation

- `CR04AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `CR04AutomaticAbility2`: [自动]（2）, automatic offer with another player's payment/movement choice.
- `CR04AutomaticAbility3`: [自动]（3）, Ave Mujica proximity mode and crystal-source restriction.
- `CR04ActiveAbility1`: [主动]（1）, own movement approach, resource cost, and target choice.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPlayerPayPlayer` and nested `PlayerRelocationController.ServerTryMovePlayer` for the one-step approach. Missing: fire pots, shortest-path ties, settlement continuation, Ave state proximity rules, and shuffle-only crystals.

### 若叶睦（CRYCHIC） — 精致的人偶

[资源]：

（1）[火罐]：初始1，上限1。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

（2）游戏开始时：自己[指定]1名其他玩家。那名玩家[破产]时，自己可重新[指定]1名玩家。

（3）自己的乐队技能为“Ave Mujica”时：回合结束时手牌数至少为3则进入[人偶模式]，严格小于3则退出[人偶模式]；只有洗牌时才能为自己的乐队技能卡添加[奇迹水晶]。

[主动]：

（1）自己的移动阶段开始时：可使用1个[火罐]，使此次用于确定移动步数的[投掷]结果，等于此角色[自动]（2）[指定]玩家上一次用于确定移动步数的[投掷]结果。

#### Script allocation

- `CR05AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `CR05AutomaticAbility2`: [自动]（2）, initial linked-player selection and optional bankruptcy reselection.
- `CR05AutomaticAbility3`: [自动]（3）, hand-count mode and crystal-source restriction.
- `CR05ActiveAbility1`: [主动]（1）, linked-player roll copying and resource cost.

#### Implementation APIs

- Related call: `EffectReactionController.RequestPlayerTargetSelection`. Missing: recorded player's last movement result, result replacement, fire pots, elimination retargeting, hand-count-driven Ave states, and shuffle-only crystal grants.

### 长崎素世（CRYCHIC） — 雨中祈晴

[主动]：

（1）其他玩家在自己拥有的格子上[结算]时：可取消那次[支付]，使自己的下回合[主要移动]改为[传送]至触发此技能的格子。

#### Script allocation

- `CR06ActiveAbility1`: [主动]（1）, optional payment cancellation and next-main-movement teleport.

#### Implementation APIs

- Calls: `PaymentController.ServerTryRequestMoneyChangeCancellation` and a future `MovementPlanController.ServerSetMovementPlan` with a teleport plan. Missing: rent-origin correlation and next-own-turn application without setup resetting the stored override.

## Cards

### 想要成为人类

（1）将此卡置于场上，从1至20间选择并声明X，将其写下。每当用于确定你的移动步数的[投掷]结果小于等于X，为此卡添加1个[奇迹水晶]。

（2）[自动]你的回合开始时，若此卡至少拥有2个[奇迹水晶]：依次进行以下操作：

1. 移除此卡上的全部[奇迹水晶]。
2. 使下次用于确定你的移动步数的[投掷]结果额外增加20-X。

若该次移动过程中受到[异常移动效果]影响，为此卡添加2个[奇迹水晶]。

#### Script allocation

- `CR07Effect1`: entire （1）, field placement, declared X, and roll-triggered crystals.
- `CR07Effect2`: entire （2）, automatic threshold spending, next-roll bonus,
  and abnormal-movement crystal restoration.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for placement. Missing: declared integer X, crystals, one-use final-roll bonus, abnormal-movement provenance, and scoped refunds after the affected movement.

### 春日影

（1）[特]抽到此卡时，若你的总资产大于等于20000：可选择使此卡直接从抽卡区打出，依次抽卡直至你的手牌数为6。若受到弃卡效果，中断此效果。

（2）此卡可作为[反击]打出。使用1次CRYCHIC角色的技能。

#### Script allocation

- `CR08SpecialEffect1`: entire （1）, draw/asset condition, deck-origin play,
  repeated draws, and discard-interruption rule.
- `CR08Effect2`: entire （2）, reaction-use permission and CRYCHIC ability invocation.

#### Implementation APIs

- Call: sequential `DrawCardController.ServerTryBeginDrawCards` while the hand remains below six. Missing: deck-draw interception, asset qualification, stop-on-discard detection, and reusable CRYCHIC character-skill dispatch.

### 去唱卡拉ok吧

进行至多5次[投掷]，并选择其中1个结果，作为本回合用于确定你的移动步数的[投掷]结果。此次移动视为正常[投掷]移动。

#### Script allocation

- `CR09PlayEffect`: entire bounded roll sequence and chosen main-movement result.

#### Implementation APIs

- Call: sequential `DiceController.ServerTryBeginRoll` up to five times. Missing: continue/stop/result choice and integration of the selected result as the normal main-movement roll rather than five separate movements.

### 想要抓住...

（1）获得1层[停留]。

（2）你的下回合开始前，当第一位其他玩家[经过]你时：在那名玩家[结算]前，你立刻向前移动1格并[结算]。

（3）你的下回合开始前，若没有玩家[经过]你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右[直线距离]最近的格子（可穿过地图，不可为自身所在格）并[结算]，视为你的[主要移动]。

#### Script allocation

- `CR10Effect1`: （1）, stay grant.
- `CR10Effect2`: （2）, first-other-player-pass response and extra movement/settlement.
- `CR10Effect3`: （3）, no-pass fallback axis teleport and main movement.
- Share activation deadline and pass history; effects 2 and 3 must not both
  treat the same activation as their eligible case.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks`, nested movement/teleport,
  and `TileResolutionController.ServerTryBeginResolution`. S05 supplies
  `MatchBoardStateData.FindTilesAlongBoardAxis` with `BoardAxisSelection.Nearest`
  for explicit Up/Down/Left/Right queries: skip gaps, exclude origin, no wrapping.
  Missing: first-passer scheduling, deadline tracking, authenticated destination
  choice, and fallback main-movement substitution.

### 优雅的呐喊

[反击]你在自己的回合外受到抽卡效果时：可打出此卡，在你的下回合结束时抽1张卡。

#### Script allocation

- `CR11PlayEffect`: entire off-turn draw reaction and scheduled later draw.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` at the next own-turn end. Missing: out-of-turn draw-effect eligibility and a one-shot delayed trigger; distinguish scheduling this draw from replacing the triggering draw.

### 是我自己的问题

[反击]

（1）[主要移动]结束时、[结算]前：打出此卡，使自己额外远离[绝对距离]最近的玩家1格。若距离最近的玩家在身前，向后移动；若与其他玩家[重叠]，可选择任意方向。

（2）若受到[异常移动效果]影响，此卡不生效。

#### Script allocation

- `CR12ReactionEffect1`: entire （1）, nearest-player check and movement away.
- `CR12Effect2`: （2）, abnormal-movement invalidation gate for this card.

#### Implementation APIs

- Call: `PlayerRelocationController.ServerTryMovePlayer` with the chosen signed step before settlement. Missing: nearest-player/tie choice, abnormal-movement history, and continuation of the original settlement exactly once.
- S05 `MatchBoardStateData.GetNearestPlayersSnapshot` supplies nearest candidates
  and retains ties; `GetBoardDistance` supplies forward/backward distances.
  The ability still owns target/direction choices and settlement continuation.

### 如果能一直持续下去...

（1）[手]依次进行以下操作：

1. 移除你乐队技能卡上的[奇迹水晶]。
2. 获得2000+500×X资金；X为上述操作移除的[奇迹水晶]数量。

（2）[持续]你的手牌数大于等于7时：立即将此卡置入弃卡区。

#### Script allocation

- `CR13HandEffect1`: （1）, band-crystal removal and money formula.
- `CR13FieldEffect2`: （2）[持续], high-hand-count discard.
- Field placement is not stated; retain effect 2's separate boundary
  without inventing a placement operation.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney` and `CardZoneController.ServerTryMoveCardToEnd` for the hand-threshold discard. Missing: band-crystal removal and clarification of how this card enters/remains in Field for its continuous effect.

### 一起演奏音乐的命运共同体

（1）[手]直到你的下回合开始，每当场上任意格子发生1次收款时：你移动到前方下一个属于行动序列后一名玩家的格子，不触发[结算]。

（2）若此卡打出后（1）效果未产生作用，将此卡返回手牌，不触发乐队技能的[自动]（3）效果。

#### Script allocation

- `CR14HandEffect1`: entire （1）, timed charge-triggered movement grant.
- `CR14Effect2`: （2）, no-impact return-to-hand and band [自动]（3） trigger exclusion.
- Share deadline and successful-activation history; no field placement is stated.

#### Implementation APIs

- Calls: `PlayerRelocationController.ServerTryMovePlayer` without explicit settlement and `CardZoneController.ServerTryMoveCardToEnd` for return to hand. Missing: receipt attribution, next-player property search, expiry, and exclusion from the band's ineffective-discard trigger.

### 主唱太拼命了

[反击]一次性向其他玩家[支付]至少5000资金时：免除此次[支付]。

#### Script allocation

- `CR15PlayEffect`: entire threshold-payment reaction and payment exemption.

#### Implementation APIs

- Call: `PaymentController.ServerTryRequestMoneyChangeCancellation` at the matching transfer cancellation stage. Define whether the 5000 threshold applies before splitting or to a specific pair, then use the corresponding operation/pair identifiers.

### 初演大成功

打出此卡后，本回合内你的资金不会下降，但拍卖与写明不受资金变动效果影响的情况除外。

回合结束后：获得1层[眩晕]，并向乐队技能卡添加1个[奇迹水晶]。

#### Script allocation

- `CR16PlayEffect`: entire turn-scoped balance protection, exceptions,
  and turn-end stun/band-crystal consequences.

#### Implementation APIs

- Calls: payment cancellation APIs and `StatusController.ServerModifyStatusStacks` for delayed stun. Missing: turn-scoped protection, auction/unmodifiable-operation exclusion, and band-crystal grant; do not apply cancellation to exempt money changes.

### [若叶睦]从没有觉得...

（1）打出此卡时，[使用者]可以选择（2）或（3）效果之一发动。

（2）使用乐队技能卡上的3个[奇迹水晶]；不足3个时，改为全部使用。然后依次进行以下操作：

1. 立即执行乐队技能的[自动]（2）效果。
2. 弃1张卡。

（3）依次进行以下操作：

1. [移除]此卡，并向抽卡区加入1张“表演的本能”。
2. 将手牌与弃卡区的全部卡放入抽卡区，并洗切。
3. 抽2张卡。

你本回合的移动以“CiRCLE”为起点，不触发起点地块效果。

#### Script allocation

- `CR17Effect1`: （1）, play-time choice of the explicitly referenced effect 2 or 3.
- `CR17Effect2`: entire （2）, crystal cost, band [自动]（2） invocation, and discard.
- `CR17Effect3`: entire （3）, removal, replacement-card creation,
  hand/discard recycle, draw, and origin replacement.
- Effects 2/3 resolve only through the chosen branch of the same activation;
  they are not separately offered as additional card plays.

#### Implementation APIs

- Calls: card removal/creation/zone APIs, discard/hand recycling, and `DrawCardController.ServerTryBeginDrawCards`. Missing: forced band transformation, crystal cost, discard choice, and silent CiRCLE movement-origin replacement.

### [高松灯]内心的呐喊

（1）打出此卡时：你可重置1次“想要成为人类”所声明的X，不影响[奇迹水晶]，并获得两个X的差值×120的资金。

（2）此卡不会被你乐队技能的[自动]（2）效果[移除]。该效果发动时，展示此卡并将其洗入抽卡区。

（3）若你的乐队技能为“MyGO!!!!!”：打出此卡时，将此卡放置于你最早拥有的格子上。你位于此卡前后5格内时，可在时机合适时使用1个[火罐]，发动1次高松灯（MyGO!!!!!）的[主动]（1）技能。

#### Script allocation

- `CR18Effect1`: （1）, declared-X reset and difference reward.
- `CR18Effect2`: （2）, band [自动]（2） removal exception and reveal/shuffle replacement.
- `CR18Effect3`: entire （3）, MyGO-conditional placement and proximity access to Tomori's [主动]（1）.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney`, `CardZoneController.ServerTryMoveCardToEnd`, and `DeckController.ServerShuffleDeck`. Missing: shared declared-X editing, transformation-removal exemption, earliest-property history, and conditional MyGO skill dispatch/fire-pot costs.

### [丰川祥子]带领着大家

回合开始时，若你与其他玩家[重叠]：可打出此卡，并记录那些玩家。你的下次[主要移动]结果对那些玩家一起执行。

你先进行[结算]，此后其他玩家按行动顺序依次进行[结算]。上述[结算]中进行的[支付]金额减半。

#### Script allocation

- `CR19PlayEffect`: entire turn-start overlap snapshot, shared next movement,
  ordered settlements, and payment reduction.

#### Implementation APIs

- Building blocks: `MatchBoardStateData.GetPlayersAtTileSnapshot` over an S04
  action-ordered candidate snapshot, sequential relocation,
  `TileResolutionController.ServerTryBeginResolution`, and payment halving.
  Missing: copying one main-movement outcome and user-first/action-order settlement
  without duplicate normal settlement.

### [长崎素世]回到曾经

（1）[特]抽到此卡时，立刻从抽卡区打出，并选择以下一个操作执行：

- 将你的所有手牌放入弃卡区，然后获得弃卡数×500的资金，抽1张卡，为你的1个格子付费加盖1间房屋，然后[移除]此卡。
- 获得1000资金，并将此卡加入手牌。

（2）[手]获得500资金。

#### Script allocation

- `CR20SpecialEffect1`: entire （1）[特], draw interception and either listed branch.
- `CR20HandEffect2`: （2）[手], 500 money.
- The two alternatives inside effect 1 are not separate effects.

#### Implementation APIs

- Calls: card-zone movement, `PaymentController.ServerTryPrintMoney`, `DrawCardController.ServerTryBeginDrawCards`, `TileController.ServerTryResolvePropertyDevelopment`, and card removal. Missing: on-draw branch dispatch, private discard count/choice, and ordered construction selection.

### [椎名立希]即便比不上...

[反击]进入移动阶段后、[结算]前：可打出此卡，重新[投掷]1次。你可继续重新[投掷]，直至结果与本回合内你[投掷]过的所有结果都不同为止。

#### Script allocation

- `CR21PlayEffect`: entire pre-settlement reaction and rerolls against
  the turn's recorded dice results.

#### Implementation APIs

- Related call: `DiceController.ServerTryBeginRoll`. Missing: this-turn result history, optional repeated rerolls, replacement of the pending movement result, and termination behavior if no distinct result remains possible.
