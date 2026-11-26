# Hello, Happy World!

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
HH-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 传播笑容

[自动]：

（1）自己拥有对应[X玩家标记（HHW）]的玩家的格子[收取]资金时：移除此卡的1个对应的[X玩家标记（HHW）]，记录此次[收取]的金额（可累加）。

[主动]：

（1）自己向其他玩家的格子[支付]资金时：可选择[支付]双倍金额。若如此做，获得一个那名玩家的[X玩家标记（HHW）]。

（2）自己盖房时：可依次进行以下操作：

1. 从盖房花费中减去此卡记录的金额（最低为0）。
2. 将此卡记录的金额归零。

#### Script allocation

- `HH01AutomaticBandEffect1`: [自动]（1）, recorded-player revenue tracking and crystal consumption.
- `HH01ActiveBandEffect1`: [主动]（1）, optional doubled payment, player record, and crystal.
- `HH01ActiveBandEffect2`: [主动]（2）, optional recorded-value construction discount and reset.

#### Implementation APIs

- Calls: `PaymentController.ServerTryMultiplyMoneyAmount` (2×) and `ServerTryAddMoneyAmountDifference` for an authorized construction discount. Missing: recorded-player crystals, actual-receipt accumulation, discount consumption, and overflow/zero-floor policy.

## Characters

### 弦卷心 — 弦卷集团

[自动]：

（1）游戏开始时：自己[获得]1000资金。

（2）自己[经过]CiRCLE时：自己[获得]1500资金。

#### Script allocation

- `HH02AutomaticAbility1`: [自动]（1）, initial money.
- `HH02AutomaticAbility2`: [自动]（2）, extra CiRCLE reward.

#### Implementation APIs

- Call: `PaymentController.ServerTryPrintMoney` for setup 1000 and each qualifying CiRCLE-pass bonus 1500. Author distinct setup/pass responders and prevent duplicate grants.

### 松原花音 — 真正的迷子

[自动]：

（1）游戏开始时：自己的起点为“弦卷豪宅”（30号格子）。

（2）自己的移动阶段：进行2次用于确定移动步数的[投掷]，将两次结果相减，作为自己的移动格数（若为负数则反向移动）。

（3）如果自己在三个回合内没有领取[CiRCLE奖励]：获得[呼诶诶]状态。

[呼诶诶]状态：用于确定移动步数的[投掷]改为1d20+1d4。领取CiRCLE奖励后移除该状态。

#### Script allocation

- `HH03AutomaticAbility1`: [自动]（1）, initial location.
- `HH03AutomaticAbility2`: [自动]（2）, two rolls and subtraction.
- `HH03AutomaticAbility3`: [自动]（3）, reward-history-dependent next movement roll.

#### Implementation APIs

- Calls: sequential `DiceController.ServerTryBeginRoll` and signed `PlayerRelocationController.ServerTryMovePlayer`. Missing: custom setup position, two-roll main-movement integration, reward-history tracking, and the alternate 1d20+1d4 rule.

### 北泽育美 — 全垒打！

[资源]：

（1）角色持有的[可乐饼]：持有上限10。

格子上的[可乐饼]属于格子状态；角色持有的[可乐饼]属于资源。从格子拾取时，减少对应数量的格子状态并增加等量角色资源；角色之间转交仍为资源转移。

[自动]：

（1）自己的每个回合开始时在“北泽精肉店”生成1个[可乐饼]。其他角色[经过]格子上的[可乐饼]时，可将其中一个拾取为那名角色持有的资源，并[支付]自己50×X资金；X为那名角色持有的[可乐饼]数量。

（2）角色每拥有1个[可乐饼]，用于确定移动步数的[投掷]额外增加1个1d2。其他角色拥有4个[可乐饼]时，自动移除其所有[可乐饼]并使其获得一层[停留]。

[主动]：

（1）自己[经过]格子上的[可乐饼]时：可将其中任意个拾取为自己持有的资源。如此拾取时，可用其中1个[可乐饼]替换自己场上1个不位于[持续]卡上的其他标记。

（2）自己[经过]其他角色时：可将自己身上的任意个[可乐饼]转移给那名角色。

（3）自己的运营阶段：可使用4个[可乐饼]，自己[获得]1000资金。

#### Script allocation

- `HH04AutomaticAbility1`: [自动]（1）, croquette generation and other-player collection offer/payment.
- `HH04AutomaticAbility2`: [自动]（2）, croquette dice modifier and other-player threshold consequence.
- `HH04ActiveAbility1`: [主动]（1）, own collection and optional marker substitution.
- `HH04ActiveAbility2`: [主动]（2）, passing-player croquette transfer.
- `HH04ActiveAbility3`: [主动]（3）, croquette spending for money.
- Initial resource/counter values above are shared source-associated metadata, not an extra activated effect.
- Collection payment samples X at the rule-defined point; the original wording does not settle pre/post-transfer sampling.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPlayerPayPlayer`, `ServerTryPrintMoney`, `MovementPlanController.ServerModifyMovementDice`, and `StatusController.ServerModifyStatusStacks`. Missing: tile/player croquette storage, transfer/caps, marker replacement, and automatic four-token consumption.

### 奥泽美咲 — 另一个我

[资源]：

（1）[火罐]：初始0，上限2。

[自动]：

（1）自己每次[经过]“弦卷集团”（#29）后：获得2个[火罐]。若自己[传送]时，CiRCLE位于自己正向移动的[移动起点]与[移动终点]之间，则在自己下次[经过]“弦卷集团”前，可将“弦卷集团”额外视为CiRCLE。

[主动]：

（1）自己的回合开始时：可使用1个[火罐]，使本次用于确定移动步数的[投掷]额外增加1d10，且此次移动改为[传送]到移动终点。

（2）自己使用[火罐]进行用于确定移动步数的[投掷]后、[结算]前：可使用1个[火罐]，获得1层在自己的下回合开始前移除的[除外]。

#### Script allocation

- `HH05AutomaticAbility1`: [自动]（1）, fire pots and teleport-triggered optional temporary CiRCLE equivalence.
- `HH05ActiveAbility1`: [主动]（1）, own-turn cost, added die, and teleport movement.
- `HH05ActiveAbility2`: [主动]（2）, post-roll cost and temporary banishment.

#### Implementation APIs

- Calls: `MovementPlanController.ServerModifyMovementDice` and `StatusController.ServerModifyStatusStacks` for authored banishment. Missing: fire pots, dice-to-teleport conversion, temporary player-specific CiRCLE equivalence, and pre-settlement banishment/expiry.
- S05 `PlayerRelocationController.ServerTryMovePlayerOffBoard` is the gameplay
  banishment-entry building block; a status stack alone does not replace movement.
  The ability still owns timing, expiry, return destination, and eligibility.

### 濑田薰 — 梦幻的王子殿下

[资源]：

（1）[火罐]：初始0，上限7。

[自动]：

（1）自己每次[经过]或被[经过]时：若场上不存在[怪盗标记]，则获得1个[火罐]；若场上存在[怪盗标记]，则移除场上的1个[怪盗标记]。

（2）自己非[传送]的[主要移动]不会[经过]移动阶段开始时场上拥有[怪盗标记]的玩家所拥有的格子。

[主动]：

（1）自己的运营阶段：可使用7个[火罐]，依次进行以下操作：

1. 在与自己[绝对距离]最近的1名玩家场上放置3个[怪盗标记]。
2. 使此次移动以“微笑号”为起点。

#### Script allocation

- `HH06AutomaticAbility1`: [自动]（1）, pass triggers, resource gain, or marker removal.
- `HH06AutomaticAbility2`: [自动]（2）, traversal exclusion.
- `HH06ActiveAbility1`: [主动]（1）, cost, nearest-player markers, and movement origin.

#### Implementation APIs

- No complete API: implement global thief-marker ownership/counts, fire-pot recharge/costs, nearest-player targeting, silent origin replacement, and traversal that skips the marked players' properties.

## Cards

### 因为我一直相信着你

至少还有1张其他手牌时：可[消耗]800资金，依次进行以下操作：

1. 公开你的卡组与手牌。
2. 选择1名玩家，由那名玩家从你的手牌中选择1张卡，放入弃卡区。
3. 由那名玩家从你的卡组中选择2张卡，加入你的手牌。
4. 重洗你的抽卡区。

#### Script allocation

- `HH07PlayEffect`: entire cost, reveal, delegated card choices, and shuffle sequence.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney`, `CardZoneController.ServerTryMoveCard`, and `DeckController.ServerShuffleDeck`. Missing: authorized hand/deck reveal and another player's validated card choices; direct deck retrieval is not automatically a draw.

### 笑容大游行

（1）[反击][经过]“弦卷集团”（29号格子）时：可将此卡放置在该格子上，当次移动的[移动终点]视为“弦卷集团”地产商，并为此卡添加3个[奇迹水晶]。你的每回合结束时，移除此卡的1个[奇迹水晶]。此卡的[奇迹水晶]为0时，若此卡仍位于“弦卷集团”，将此卡放入弃卡区。

（2）[持续][结算]后：可将“弦卷集团”格子上的此卡放置于[移动终点]格子上，并移除此卡上的全部[奇迹水晶]。

（3）[持续]此卡位于格子上时，该格子视为与“弦卷集团”格子交换位置。任何玩家在此卡放置的格子上[结算]后，将此卡放入弃卡区。

#### Script allocation

- `HH08Effect1`: entire （1）, reaction placement, destination, crystals, and decay.
- `HH08FieldEffect2`: （2）, post-settlement relocation and crystal removal.
- `HH08FieldEffect3`: entire （3）, board-position exchange and settlement discard.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for tile placement/repositioning/discard. Missing: crystal lifetime, active-movement destination replacement, and reversible board-position swapping; changing tile type is not swapping board positions.

### 出发！后台之旅！

依次进行以下操作：

1. 观看牌堆顶部2张卡。
2. 选择其中0至2张卡，以任意顺序放回；将剩余的卡翻入弃卡区。
3. 每翻入1张卡，获得1000资金。

#### Script allocation

- `HH09PlayEffect`: entire inspection, ordered return/discard choice, and money.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCard` for ordered deck return/discard and `PaymentController.ServerTryPrintMoney` for 1000 per discarded card. Missing: owner-private top-two inspection and ordered selection.

### [濑田薰]怪盗hello happy

将此卡放置于场上（充能3，衰减1），并向1名玩家的场上放置1个[怪盗标记]。你本回合的移动阶段，可以选择在[经过]那名玩家时使自己[强制停下]，并进行[结算]。

此卡在场上时，所有位于薰所在格子的玩家，如果可以移动，则其[主要移动]改为[投掷]1d2确定前后方向、[投掷]1d10确定距离，并进行[结算]。

#### Script allocation

- `HH10PlayEffect`: placement/charge setup, target marker, and this-turn stop option.
- `HH10FieldEffect`: charge decay/expiry and the field-active movement replacement
  for eligible players on Kaoru's tile. Preserve unresolved charge shorthand.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `DiceController.ServerTryBeginRoll` for direction/distance. Missing: charge decay, thief markers, force-stop, movement eligibility, and replacement of affected players' main-movement pipelines.

### 微笑巡逻队

依次进行以下操作：

1. 付款，在任意1个自己的格子加盖1层房屋。
2. [投掷]3d20。
3. 若结果数字对应的格子可建造，在该格子额外免费加盖1层房屋。

#### Script allocation

- `HH11PlayEffect`: entire paid construction, roll, and conditional free construction.

#### Implementation APIs

- Calls: `TileController.ServerTryResolvePropertyDevelopment`, `DiceController.ServerTryBeginRoll` (3d20), then authorized `ServerDevelopTile` and `ServerSetDefaultRent`. Validate both construction targets/caps and distinguish paid construction from the free bonus.

### [弦卷心]前往笑容集结的地方！

[支付]10000资金，视为买地花费，并将此卡置于CiRCLE上。

其他玩家在该格子[结算]时：向所有者[支付]6000资金，视为该格子的收款。

#### Script allocation

- `HH12PlayEffect`: purchase-classified payment and placement on CiRCLE.
- `HH12FieldEffect`: other-player settlement charge treated as tile revenue.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney`, `CardZoneController.ServerTryMoveCardToEnd`, and `PaymentController.ServerTryPlayerPayPlayer` (6000). Missing: purchase-cost attribution and property-rent classification for a card-created CiRCLE charge.

### 黑衣人的补给

[反击][经过]“CiRCLE”（1号格子）时：可打出此卡，在“弦卷集团”（29号格子）上放置1个[奇迹水晶]。

“弦卷集团”格子上拥有[奇迹水晶]时，该格子获得“CiRCLE”格子的全部效果。你[经过]“弦卷集团”格子后，移除该格子的1个[奇迹水晶]。

#### Script allocation

- `HH13PlayEffect`: entire pass reaction and crystal placement; installs the
  crystal-dependent tile behavior and its pass-consumption rule.
- The ongoing behavior belongs to the affected tile/marker state, not an
  unstated field-card source. Its source binding remains to be implemented.

#### Implementation APIs

- No complete API: implement tile-scoped crystals and a temporary overlay of CiRCLE's full behavior on the named tile. `TileController.ServerSetTileType` is not an additive, source-scoped behavior overlay.

### 运动的天赋

将此卡放置于自己的场上，并在此卡上放置3个[奇迹水晶]。每回合结束时，失去此卡的1个[奇迹水晶]；数量为0时，将此卡置入弃卡区。

此卡在场上时：

- 每次[投掷]，获得1次资金。起始金额为700，每次减少100，奖励下限为100。
- 每次进行用于确定移动步数的[投掷]时，重新[投掷]，直至结果至少为10。

#### Script allocation

- `HH14PlayEffect`: field placement and three-crystal initialization.
- `HH14FieldEffect`: decay/discard, diminishing roll rewards, and movement rerolls.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryPrintMoney`, and `DiceController.ServerTryBeginRoll`. Missing: crystal lifetime, decreasing reward state, and a bounded/reaction-safe reroll loop integrated with the original movement roll.

### Happy, Lucky, Smile, Yeah！

[投掷]1d4mod4，对应结果1至4，沿上、下、左、右中的对应方向，[传送]至[直线距离]最远的格子。例：在弦卷豪宅[投掷]出4时，[传送]到商店街。此次[传送]视为你的[主要移动]。

#### Script allocation

- `HH15PlayEffect`: entire roll, axis/destination selection, and teleport main movement.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (1d4),
  `MatchBoardStateData.FindTilesAlongBoardAxis` with an explicit
  `BoardAxisDirection` and `BoardAxisSelection.Farthest`, and
  `MovementPlanController.ServerSetMovementPlan` with a teleport plan.
  The axis query uses authored coordinates, skips gaps, excludes origin, and does
  not wrap. Missing: main-movement entry; resolve modulo-zero versus 1–4 mapping.

### 热气球演出

依次进行以下操作：

1. [投掷]4次3d20mod60，并记录各次结果。
2. 选择其中1个结果，[传送]至该结果对应序号的格子。

此次[传送]视为你的[主要移动]。

#### Script allocation

- `HH16PlayEffect`: entire four-roll sequence, result choice, and teleport main movement.

#### Implementation APIs

- Calls: four sequential `DiceController.ServerTryBeginRoll` operations (3d20), then `MovementPlanController.ServerSetMovementPlan` with the chosen teleport. Missing: result choice and main-movement entry; define modulo-zero to tile-index conversion.

### 爱心义演

打出此卡的回合内：

- 你进行[投掷]移动时，每初次[经过]1个属于你的格子，使你的总移动数增加2。
- 你向其他玩家[支付]时，付款金额减半，向上取整10。

若弦卷心已将专属卡置于CiRCLE上，CiRCLE也算作属于弦卷心的格子。

#### Script allocation

- `HH17PlayEffect`: entire turn-scoped movement-extension and payment-reduction
  grant, including the stated CiRCLE ownership exception.

#### Implementation APIs

- Call: `PaymentController.ServerTryMultiplyMoneyAmount` (1/2) for eligible outgoing transfers. Missing: live remaining-step extension, first-pass-per-property history, temporary ownership equivalence, and turn-scoped expiry.

### [松原花音]Wacha Mocha 啪嗒进行曲

[持续]花音每次倒走时，为此卡添加1个[水母标记]。

运营阶段中，若此卡至少有9个[水母标记]：可以清除所有标记，依次进行以下操作：

1. [传送]到“水族馆”（4号格子）或“弦卷豪宅”（30号格子），视为本次[主要移动]（喊出呼诶诶～！）。
2. 将此卡置入弃卡区。

#### Script allocation

- `HH18FieldEffect`: entire [持续] block, marker gain, threshold/cost, teleport,
  and discard. The text does not specify a separate placement effect.

#### Implementation APIs

- Calls: `MovementPlanController.ServerSetMovementPlan` with a teleport plan and `CardZoneController.ServerTryMoveCardToEnd` to Discard. Missing: backward-movement history, jellyfish counters, destination choice, and main-movement entry.

### [北泽育美]We Can☆Hurray!

（1）[手][投掷]2次4d20，选择其中1个结果：

- 若选择的结果大于60：获得1000资金并抽1张卡，不放置标记。
- 否则：将1个[育美标记]放置到该结果对应的格子上。你[经过]该标记时，可在该格子[强制停下]并获得2000资金，然后移除该标记。

（2）[反击]在你回合外收到资金的回合结束时，可打出此卡。当前回合内，你每获得过1次资金，此卡的[投掷]次数增加1。

#### Script allocation

- `HH19HandEffect1`: entire （1）, rolls, result choice, marker/reward branch,
  and the marker's later stop/reward behavior.
- `HH19ReactionEffect2`: （2）, off-turn reaction permission and additional roll count.
- Both use the same play context; effect 2 modifies effect 1 rather than
  performing an extra independent card play.

#### Implementation APIs

- Calls: sequential `DiceController.ServerTryBeginRoll` (4d20), `PaymentController.ServerTryPrintMoney`, and `DrawCardController.ServerTryBeginDrawCards`. Missing: income-history roll count, result choice, tile markers, and force-stop.

### [奥泽美咲]unnamed

[反击]使用[火罐]进行用于确定移动步数的[投掷]后、[结算]前：可打出此卡，使用所有[火罐]，依次进行以下操作：

1. 选择1名位于你的[移动起点]与预定[移动终点]之间的玩家，[传送]至那名玩家所在的格子，并进行[结算]。此次[传送]视为你的[主要移动]。
2. 那格及相邻2格上的所有其他玩家各[支付]你500资金。

#### Script allocation

- `HH20PlayEffect`: entire Misaki reaction, fire-pot cost, intermediate-player
  choice, main-movement teleport/settlement, and nearby-player payments.

#### Implementation APIs

- Calls: movement-plan/relocation building blocks and `PaymentController.ServerTryPlayerPayPlayer` with total 500 × eligible payer count. Missing: fire-pot consumption, original-path target choice, active main-movement replacement, and settlement sequencing.
- S05 supplies `MatchBoardStateData.GetPotentialMovementPathSnapshot` and
  `GetPlayersAlongPathSnapshot` (at most 255 entries), plus
  `GetPlayersWithinDistanceSnapshot` for nearby payers. The effect must capture
  the original movement context and validate its choices. Querying the potential
  path does not execute step movement or add passing reactions along that path.

### 梦幻的回礼

[反击]场上其他玩家即将被不属于你的格子收费时：打出此卡，向场上你以外任意1名玩家的1个格子进行1次[支付]。

若成功进行[支付]：自动使用1次你的乐队技能进行双倍[支付]，为乐队技能卡添加2个分别记录这两名玩家的[奇迹水晶]，并为将要进行[支付]的那名玩家减免等于你[支付]金额的资金。

#### Script allocation

- `HH21PlayEffect`: entire reaction, selected payment, conditional band activation,
  two player records/crystals, and intercepted-payment reduction.

#### Implementation APIs

- Related calls: `TileController.ServerTryStartPropertyRentPayment` and payment multiply/additive modifiers. Missing: a nested payment sequence while another payment is pending, band-effect dispatch, two-player crystal provenance, and discount based on the actual payment.
