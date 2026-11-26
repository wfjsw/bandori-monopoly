# MyGO!!!!!

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
MG-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 迷途之星

[资源]：
（1）[奇迹水晶]：初始0，上限1。
（2）来自“迷途之星”[自动]（3）的临时[奇迹水晶]：初始0，上限2。
[自动]：

（1）游戏开始时：自己[投掷]3d20，以结果对应的格子作为本局游戏的起点。

（2）自己的移动[投掷]结果至少为16时：为此卡添加1个[奇迹水晶]。

（3）自己的卡在未生效的情况下进入弃卡区时：为此卡添加1个临时[奇迹水晶]。

[主动]：

（1）自己的运营阶段中：可选择使用此卡的1个[奇迹水晶]，立刻进入移动阶段并将本回合的主要移动改为“正向移动1格”。

（2）自己的运营阶段中：可选择使用此卡的2个[奇迹水晶]，自己[抽卡]1张。

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MG01AutomaticBandEffect1`: [自动]（1）, initial roll and location.
- `MG01AutomaticBandEffect2`: [自动]（2）, movement-roll crystal gain.
- `MG01AutomaticBandEffect3`: [自动]（3）, unsuccessful-card crystals and overflow allowance.
- `MG01ActiveBandEffect1`: [主动]（1）, fixed-step substitution and crystal removal in written order.
- `MG01ActiveBandEffect2`: [主动]（2）, crystal spending for a draw.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll`, `MovementPlanController.ServerSetMovementPlan` with a one-step plan, and `DrawCardController.ServerTryBeginDrawCards`. Missing: custom starting position, band crystals/overflow, ineffective-card outcomes, and resource-cost integration.

## Characters

### 高松灯 — 诗超绊

[资源]：
（1）[火罐]：初始4，上限4。
[自动]：

（1）自己[经过]任意RiNG时：获得1个[火罐]。

[主动]：
（1）其他玩家的[移动终点]与自己所在格子的[绝对距离]不多于5时：可使用4个[火罐]并依次进行以下操作：
  1. 在那名玩家[结算]前，将其[传送]至自己所在的格子并[结算]。
  2. 令其[投掷]1d20，若结果小于等于6，则那名玩家在获得1层“回合结束后获得1层[停留]和1层[重力]并减少1层。”的[延迟[停留]，[重力]]状态。（[重力]是“拥有此状态时即使同时拥有[停留]也可正常[结算]，且[结算]流程中的分摊后[支付]变成0.5倍。回合结束时减少1层。”）

（2）自己的[移动终点]与任意其他玩家所在格子的[绝对距离]不多于5时：可使用4个[火罐]，在自己的[结算]前，将自己[传送]至任意满足上述条件的玩家所在的格子并[结算]。

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MG02AutomaticAbility1`: [自动]（1）, RiNG resource gain and initial amount/cap.
- `MG02ActiveAbility1`: [主动]（1）, other-player pull, settlements, roll/stay, and payment reduction.
- `MG02ActiveAbility2`: [主动]（2）, own-arrival teleport and settlement.
- The delayed settlement occurs at the end of the affected player's own next
  turn, not Tomori's next turn (DEC-10.04).

#### Implementation APIs

- Calls: nested relocation, tile-resolution, dice, status-stack, and payment multiplier APIs. Missing: fire pots, distance choices, suppression of duplicate original settlement, delayed stay settlement, and correlation of the half-price payment scope.
- S05 `MatchBoardStateData.GetPlayersWithinDistanceSnapshot` supplies the nearby
  candidate set from an explicit origin tile; authenticated selection remains Pending.
- Scheduling requirement: bind the delayed settlement to the affected PlayerId
  and that player's next turn-end. Preserve Tomori's effect attribution
  separately; do not use the source player's next turn as the timer anchor.
  CTC-14.01 is a proposed scheduling capability, not an implemented API.
  Verify with distinct source/affected players and different turn orders.
  Implementation/runtime verification: Not Run.

### 千早爱音 — 重新开始

[资源]：
（1）[火罐]：初始2，上限3。
[自动]：

（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。

[主动]：

（1）自己回合结束时自己的[停留]减少层数时：可使用2个[火罐]，自己[抽卡]1张。
（2）自己[传送]后：可使用2个[火罐]，自己[抽卡]1张。

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MG03AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `MG03ActiveAbility1`: [主动]（1）, post-effect resource cost and draw; processing an effect does not add tile settlement.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` after the qualifying stay/teleport effect resolves. Missing: fire-pot recharge/costs and an authoritative completion signal distinguishing effect resolution from merely adding a status stack.

### 长崎素世 — 通透的颜色

[资源]：
（1）[火罐]：初始1，上限1。
[自动]：

（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。

[主动]：

（1）自己的移动[投掷]后：可使用1个[火罐]，使自己本回合的[移动终点]对自己视为对应颜色的[地产商]格子，持续至自己的下回合开始。若因此效果单次免除了至少1500资金的[支付]，则自己下次[经过]CiRCLE时不获得[火罐]。
获得1层“[移动终点]对自己视为对应颜色的[地产商]格子，如果本次移动进行了[结算]且原本[结算]流程包括分摊前需[支付]至少1500资金则获得一层[素世无火]状态。自己回合开始前减少1层。”的[素世]状态。（[素世无火]是“无视“通透的颜色”的[自动]（1）。自己[经过]CiRCLE时减少所有层数。”）

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MG04AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `MG04ActiveAbility1`: [主动]（1）, temporary destination trait and conditional next-resource suppression.

#### Implementation APIs

- No complete API: implement a temporary player-specific developer-function overlay, prevented-payment accounting, fire-pot cost, and next-CiRCLE recharge suppression. Global tile-type changes would affect the wrong players.

### 椎名立希 — 决定练习日的会议

[资源]：
（1）[火罐]：初始0，上限5。
[自动]：

（1）任何玩家获得X层[停留]时：自己获得X个[火罐]。
[主动]：

（1）自己的运营阶段且不拥有[立希]时：可使用1个[火罐]，为自己场上的任意1张卡拥有[奇迹水晶]资源的卡添加1个[奇迹水晶]并获得1层“回合结束时减少1层的”[立希]状态。

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MG05AutomaticAbility1`: [自动]（1）, stay-triggered resource gain and initial amount/cap.
- `MG05ActiveAbility1`: [主动]（1）, once-per-turn conversion to a field-card crystal.

#### Implementation APIs

- No complete resource API: observe successful stay-stack gains, maintain capped fire pots, then transfer the selected resource into one eligible field card's crystals once per own turn.

### 要乐奈 — 投币式停车场的猫

[资源]：
（1）[火罐]：初始3，上限3。
[自动]：

（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。

（2）自己在“Space”（43号格子）上[结算]时：依次进行以下操作：
  1. 如果“Space”属于其他玩家，则将本次[结算]流程中的基础[结算]改为自己[抽卡]1张。
  2. 如果自己为“Space”的拥有者，则可选择将此次将本次[结算]流程中的基础[结算]改为在该格子添加1层“使租金增加层数×500资金”的[抹茶芭菲状态]状态。

（3）其他玩家在“Space”[结算]时：则可选择将此次将本次[结算]流程中的基础[结算]改为使用的1个[抹茶芭菲资源]，此格子获得1层[抹茶芭菲状态]状态。

（4）自己与其他玩家[重叠]时：依次进行以下操作：
  1. 那名玩家[支付]自己800资金。
  2. 那名玩家获得1个[抹茶芭菲资源]。

（5）拥有[抹茶芭菲资源]的玩家[经过]任意RiNG时：可使用1个[抹茶芭菲资源]，[获得]400资金。
[主动]：
自己的运营阶段中：可选择使用此卡的1个[奇迹水晶]，立刻进入移动阶段并将本回合的主要移动改为“正向移动1格”。
（1）自己的运营阶段可：可选择使用3个[火罐]，依次进行以下操作：
  1. 获得1层“基础[结算]不可进行地契购买，[结算]后减少1层”的[无法购买地契]状态。
  2. 立刻进入移动阶段并将本回合的主要移动改为“[传送]至“Space””。

#### Script allocation

Mapping status: Pending reconciliation. Script allocation and API notes below describe the previous wording.

- `MG06AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `MG06AutomaticAbility2`: [自动]（2）, Space settlement replacement, tile parfaits, and other-player payment exemption.
- `MG06AutomaticAbility3`: [自动]（3）, overlap transfer/reward and parfait-holder pass offer.
- `MG06ActiveAbility1`: [主动]（1）, resource-funded Space teleport and purchase prohibition.
- The original unqualified Space-settlement behavior is kept separate from the teleport activation. Activation timing beyond replacing this turn's movement remains unspecified.

#### Implementation APIs

- Calls: movement-plan setters, `DrawCardController.ServerTryBeginDrawCards`,
  payment cancellation/transfer/print operations, and
  `MatchBoardStateData.GetPlayersAtTileSnapshot` for current overlap candidates.
  Missing: fire pots, player/tile parfait tokens, purchase prohibition, settlement
  replacement, trigger integration, and token-modified Space rent.

## Cards

### 即使迷茫着

（1）[反击]当你被其他人的卡的效果影响时，你将此卡放置在自己场上。（2）[持续] 主要阶段中，你可将此卡置入弃卡区并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。

#### Script allocation

- `MG07ReactionEffect1`: （1）, reaction field placement.
- `MG07FieldEffect2`: （2）, discard and hand-count main movement.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `MovementPlanController.ServerSetMovementPlan` with a fixed-step plan based on hand count. Missing: card-impact reaction eligibility, count-sampling timing, and effect-driven movement-phase entry.

### [高松灯]不再迷茫

（1）获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡置入弃卡区，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数 （2）你使用角色技能时可使用此卡上的1个[奇迹水晶]以代替此次技能的[火罐]使用，当此卡上的水晶由于此效果以外的原因减少时，此卡立刻获得等同于减少量的[奇迹水晶] （3）此卡上的奇迹水晶耗尽后，[移除]此卡。

#### Script allocation

- `MG08Effect1`: entire （1）, protected stay, field placement, hand discard, and crystals.
- `MG08Effect2`: entire （2）, ability-cost substitution and crystal-loss restoration.
- `MG08Effect3`: （3）, removal when crystals are exhausted.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks`, `CardZoneController.ServerTryMoveCardToEnd`, and `CardRemovalController.ServerRemoveCard`. Missing: naturally-decaying-only stay protection, crystal accounting/refunds, and replacing character fire-pot costs.

### 轮符雨

使自己获得一层[停留]并在回合结束时额外进行一次[结算]。

#### Script allocation

- `MG09PlayEffect`: entire stay grant and scheduled turn-end settlement.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks` for authored stay and delayed `TileResolutionController.ServerTryBeginResolution`. Missing: turn-end scheduling and the rule that permits this additional settlement despite the status.

### 壱雫空

清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。（此卡可在眩晕时打出）。

#### Script allocation

- `MG10PlayEffect`: entire status removal, per-player payments,
  user reward, and stun-use exception.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks`, `PaymentController.ServerTryPlayerPayPlayer`, and `ServerTryPrintMoney`. Count successfully removed status types rather than stacks; missing: activation while stunned and interaction with statuses protected from removal.

### 无路矢

指定场上自己以外的一位玩家所在格子，获得2层[除外]并在[除外]层数归0后[传送]至该格子，视为当回合的主要移动。[除外]期间本应获得的格子收入由此前指定的那名玩家获得。

#### Script allocation

- `MG11PlayEffect`: entire target snapshot, banishment, expiry teleport,
  and temporary income redirection. No field placement is stated.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks` and `MovementPlanController.ServerSetMovementPlan` with the recorded teleport destination. Missing: banishment-expiry scheduling, income-recipient substitution, target snapshot, and main-movement integration.

### 羽丘的不可思议女孩

位于属于自己的格子上时，投掷1d20，若出目至少为10则在当前格子免费加盖一层房屋，若出目至少为15，则额外抽一张卡，至少为20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃卡区并抵消一次任意付款。若小于10，此卡放入弃卡区且视为此卡未生效。

#### Script allocation

- `MG12PlayEffect`: eligibility, roll thresholds, construction/draw,
  conditional field placement, and failed-result discard.
- `MG12FieldEffect`: the placed card's later payment cancellation and discard.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll`, authorized `TileController.ServerDevelopTile` plus `ServerSetDefaultRent`, draw/zone APIs, and payment cancellation. Missing: stored cancellation entitlement and explicit ineffective-card outcome for the below-10 branch.

### [千早爱音]Anon Tokyo

[特]：

仅当自己所在的格子是[可购买格子]时，才可从手牌打出。

[手]：

[指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作：

1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1），被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；

2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。

#### Script allocation

- `MG13SpecialEffect`: [特], current-tile eligibility for playing from hand.
- `MG13HandEffect`: entire [手] block, target and ownership-dependent
  linking/payment or movement branch. Numbered steps remain inside it.
- Persistent link revenue behavior belongs to the property-link state,
  not an unstated field-card effect.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney` and movement-plan setters. Missing: adjacent-property choice, persistent links/crystals, linked-rent composition, and backward one-step main movement; normal forward fixed-step plans do not express every branch.
- S05 `MatchBoardStateData.GetAdjacentTileIndexes` supplies tile-order neighbors;
  property eligibility, authenticated choice, and this card's integration remain Pending.

### [长崎素世]混合的颜色

将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子[结算]的玩家处收费时，收费在地产商的减半收费基础上额外减半。

#### Script allocation

- `MG14PlayEffect`: placement on an owned property.
- `MG14FieldEffect`: all-color trait, construction restriction,
  and additional developer-mediated rent reduction.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and a correlated `PaymentController.ServerTryMultiplyMoneyAmount`. Missing: multi-color property overlays, original-color construction restrictions, and developer-origin rent classification.

### [椎名立希]想认真去做

使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[结算]，本次[结算]导致的所有[支付]变为原价的四分之一。

#### Script allocation

- `MG15PlayEffect`: entire eligible-player iteration, remote settlement choice,
  and payment reduction scoped to those settlements.

#### Implementation APIs

- Related calls: `TileResolutionController.ServerTryBeginResolution` and `PaymentController.ServerTryMultiplyMoneyAmount` (1/4). Missing: remote chosen-tile settlement; the resolution API settles the player's current position, so do not silently teleport targets.

### [要乐奈]有趣的女人

将此卡置于当前格子上，每当有人经过且未在其上[结算]时为其增加一个奇迹水晶，当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个“抹茶芭菲”或强制停下并[结算]，如果强制停下则此卡洗入弃卡区。由此卡效果导致[结算]时需支付资金减半。

#### Script allocation

- `MG16PlayEffect`: placement on the current tile.
- `MG16FieldEffect`: pass crystals, threshold choice, forced settlement,
  correlated half payment, and discard behavior.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and correlated payment halving. Missing: pass-without-settlement history, crystals/parfait choice, force-stop, and clarification of what shuffling into the discard area means.

### 那天的雨

将此卡放置于自己场上并为其放置5个奇迹水晶，打出此卡时及你的每回合开始时投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]，每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃卡区（骰点大于10则固定为高级住宅区对应颜色的格子）。

#### Script allocation

- `MG17PlayEffect`: field placement, initial crystals, and the immediate roll/application.
- `MG17FieldEffect`: later turn-start roll/applications, turn-end decay,
  and exhaustion discard. Share roll/application helpers without duplicating
  the immediate application.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `DiceController.ServerTryBeginRoll` (1d10), and `StatusController.ServerModifyStatusStacks`. Missing: five-crystal lifetime and authored developer/color/adjacency target lookup.

### 若能再次交汇

[反击]移动掷骰后，且场上有可被经过的玩家时可打出，持续进行移动掷骰直至[经过]下一名玩家（最多掷骰至移动超过原本移动终点的20格以后）。

#### Script allocation

- `MG18PlayEffect`: entire post-roll reaction and bounded repeat-roll movement.

#### Implementation APIs

- Building blocks: sequential `DiceController.ServerTryBeginRoll` and `PlayerRelocationController.ServerTryMovePlayer`. Missing: repeated-roll main-movement integration, stop-on-next-player behavior, and the original-endpoint-plus-20 stopping boundary.

### 哪怕这旅程没有终点

[手] 将此卡放置于当前格子上并为其放置4个奇迹水晶，每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹水晶数量为0时，将此卡置入弃卡区。（2）[持续] [结算]时，获得X*60资金，X为你此次主要移动[经过]的格数。

#### Script allocation

- `MG19HandEffect`: entire [手] block, placement, crystals, qualified decay, and discard.
- `MG19FieldEffect2`: （2）[持续], movement-distance settlement reward.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `PaymentController.ServerTryPrintMoney`. Missing: card-crystal decay, main-movement pass counts, and clarification of whose settlement activates the tile-placed card.

### 难以复刻的奇迹

（1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上，当你进行加盖动作时可使用此卡上的1个[奇迹水晶]以代替资金花费。（2）[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡进入弃卡区。（3）若此卡进入弃卡区时其上仍有奇迹水晶，视为此卡未生效。

#### Script allocation

- `MG20HandEffect1`: entire （1）, placement, crystal transfer, and construction-cost grant.
- `MG20FieldEffect2`: entire （2）, all-player pass tracking, reward, and discard.
- `MG20Effect3`: （3）, discard-time unsuccessful-effect classification.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `PaymentController.ServerTryPrintMoney`. Missing: crystal transfers, crystal-funded construction, per-player pass completion, closest-crystal-count tie handling, and ineffective-discard outcome tracking.

### 普通与理所当然

[反击]受到异常移动效果影响后可打出，使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。

#### Script allocation

- `MG21PlayEffect`: entire abnormal-movement reaction and next-main-movement
  replacement using the last qualifying movement's recorded distance.

#### Implementation APIs

- Call: `MovementPlanController.ServerSetMovementPlan` with a fixed-step plan if it represents the recorded move. Missing: last non-teleport main-movement history, direction policy, abnormal-effect completion detection, and one-use next-movement scheduling.
