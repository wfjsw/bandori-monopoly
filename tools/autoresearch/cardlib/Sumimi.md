# sumimi

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
SU-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 人气偶像组合

[资源]：

（1）[奇迹水晶]：初始0。

[自动]：

（1）自己的回合结束时：若自己在该回合内从其他玩家处[收取]过资金，则为此卡添加1个[奇迹水晶]；否则，移除此卡上的所有[奇迹水晶]。

（2）自己可在卡组中携带两个角色的专属卡，但只有自己的角色卡为对应角色时才可打出该专属卡。

（3）自己向其他玩家收费时：使此次收费金额额外增加X×100资金；X为此卡上的[奇迹水晶]数量。

#### Script allocation

- `SU01AutomaticBandEffect1`: [自动]（1）, own-turn receipt history, crystals, or reset.
- `SU01AutomaticBandEffect2`: [自动]（2）, deck permission and current-character use restriction.
- `SU01ActiveBandEffect1`: [主动]（1）, optional crystal-scaled charge increase; does not spend crystals.

#### Implementation APIs

- Call: `PaymentController.ServerTryAddMoneyAmountDifference` for qualifying receipts. Missing: actual-transfer turn history, crystal growth/reset, dual-exclusive deck eligibility, and character-specific play restrictions.

## Characters

### 纯田真奈 — 甜甜圈爱好者

[资源]：

（1）[火罐]：初始1，上限2。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

[主动]：

（1）自己[经过]“购物中心”、“便利店”、“快餐店”、“羽泽咖啡厅”或“山吹面包房”时：可使用1个[火罐]，依次进行以下操作：

1. 向对应格子进行1次[支付]。可向已抵押的格子正常[支付]其原本收费；若为无主格子，改为[消耗]对应收费金额。
2. 获得一层[甜甜圈]状态。

[甜甜圈]状态：自己的[消耗]与[支付]资金减半。在获得该状态后的下个回合结束时移除该状态。

#### Script allocation

- `SU02AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `SU02ActiveAbility1`: [主动]（1）, pass reaction, resource cost, special tile payment, and timed expenditure reduction.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPlayerPayPlayer`, `ServerTryDeleteMoney`, and `ServerTryMultiplyMoneyAmount` (1/2). Missing: fire pots, voluntary payment on passing including mortgaged-property valuation, and expiry after the next own turn.

### 三角初华（Sumimi） — 成为偶像

[资源]：

（1）[火罐]：初始2，上限2。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

[主动]：

（1）回合开始时：自己可使用1个[火罐]，获得一层[sumimi初华标记X]，X为与“主要街道”颜色相同的任意1个格子的编号。

其他玩家[经过][sumimi初华标记X]对应的格子时，获得一层[sumimi初华标记X（2）]状态。

[sumimi初华标记X（2）]状态：到达[移动终点]后，依次进行以下操作：

1. 自己[支付][sumimi初华标记X]的拥有者Y×30资金；Y为自己[经过][sumimi初华标记X]对应的格子后移动的剩余格数。
2. 此状态减少1层，并将对应的[sumimi初华标记X]减少1层（如果有）。

[sumimi初华标记X]：获得时使自己[火罐]上限-1，移除时使自己[火罐]上限+1,自己被由自己以外的消耗火罐效果影响时移除等量的该标记代替[火罐]消耗。

#### Script allocation

- `SU03AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `SU03ActiveAbility1`: [主动]（1）, tile resource placement and later pass/path-dependent payment and cleanup.
- The original unqualified turn-start activation timing is retained.

#### Implementation APIs

- Call: `PaymentController.ServerTryPlayerPayPlayer` after the passing player's movement completes. Missing: tile fire-pot placement, source ownership, remaining-step capture, marker consumption, and delayed payment correlation.

## Cards

### 兼顾偶像与乐队

你本回合未进行过赎回操作时：可打出此卡，将你的资金重设为3000。若你的资金在3000以上，此卡无效果。

#### Script allocation

- `SU04PlayEffect`: entire redemption-history eligibility and balance reset.

#### Implementation APIs

- Related call: `PaymentController.ServerTryPrintMoney` for the shortfall to 3000. Missing: redemption history and a decision whether the reset is event-modifiable; normal printing may not leave the balance exactly 3000.

### Sumimi不会解散哦

你本回合未进行过抵押或赎回操作，且资金数额不含相同数字时：可打出此卡，进行1次可使你的资金数额变为含有相同数字的[传送]。此次[传送]视为你的[主要移动]。

若[传送]并[结算]后，资金数额未能变为含有相同数字：回到原处，并取消所有受到的效果。

#### Script allocation

- `SU05PlayEffect`: entire history/digit eligibility, teleport choice,
  settlement, and conditional return/compensation.
- Script allocation does not resolve the required rollback semantics.

#### Implementation APIs

- No complete API: implement valid destination prediction, mortgage/redemption history, main-movement teleport, post-settlement digit validation, and rollback of all affected state. A return teleport alone cannot undo the settlement.

### 一人两个甜甜圈

（1）获得[除外]，直至你原本所在的格子被其他玩家[经过]。

（2）你原本所在的格子被其他玩家[经过]时：可在那名玩家[结算]后，选择[传送]至你原本所在格子（不包括）与那名玩家本次[移动终点]之间的任意1格，并进行[结算]。之后，你们各获得2个[火罐]；超出上限的每个[火罐]转化为500资金。

#### Script allocation

- `SU06Effect1`: （1）, banishment tied to the original tile's next pass.
- `SU06Effect2`: entire （2）, that pass's settlement continuation,
  optional destination, settlement, and both fire-pot/overflow rewards.

#### Implementation APIs

- Calls: `StatusController.ServerModifyStatusStacks`, nested teleport/settlement, and `PaymentController.ServerTryPrintMoney` for overflow. Missing: event-bound banishment expiry, recorded path choice, fire-pot grants/caps, and coordination after another player's settlement.
- S05 `PlayerRelocationController.ServerTryMovePlayerOffBoard` supplies replacement
  OffBoard entry; status/expiry/return coordination remains Pending. Potential-path
  queries do not supply the historical movement record required by this card.

### 现在她是Sumimi的小初啦

[反击]从你的本回合开始后，到下回合开始前，若你失去资金的总额即将超过你所在格子的[收费标价]：打出此卡，获得等于该格子[收费标价]的资金。RiNG取当前收费乘数，不固定使用初始值。

#### Script allocation

- `SU07PlayEffect`: entire cumulative-loss reaction and tile-price-based reward.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney` and `TileController.TryGetRingRentMultiplier` for the special Ring basis. Missing: cumulative loss tracking between own-turn starts and safe reward sequencing while a triggering payment is pending.

### Sumimi是二人一体的

（1）打出此卡时：将自己的角色卡替换为Sumimi的另一名角色，并将[火罐]数量设为该角色的初始[火罐]数量。

（2）你进行用于确定移动步数的[投掷]后，可将此卡作为[反击]使用。

#### Script allocation

- `SU08Effect1`: （1）, character replacement and initial resources.
- `SU08Effect2`: （2）, post-movement-roll reaction-use permission.
- Effect 2 permits invoking effect 1; it does not perform another swap itself.

#### Implementation APIs

- No complete in-match character-swap API: replace the Sumimi character effect source and initialize its fire pots while retaining player identity and other state. Character-selection setup is not an in-match replacement operation.

### Here the world

（1）[反击]有人在同一回合内打出2张卡时：将此卡放置于那名玩家的场上。

（2）场上有此卡的玩家下次抽卡时：将那张卡背面朝上放置于此卡上，并为其放置3个[奇迹水晶]。

那名玩家的每个回合开始时，移除1个[奇迹水晶]。[奇迹水晶]数量为0时，那名玩家将那张卡加入手牌，并使此卡的[使用者]抽1张卡。

#### Script allocation

- `SU09ReactionEffect1`: （1）, two-card-use reaction and placement.
- `SU09FieldEffect2`: entire （2）, next-draw capture, face-down attachment,
  crystals/countdown, release to hand, and user draw.

#### Implementation APIs

- Building blocks: `CardZoneController.ServerTryMoveCardToEnd` and `DrawCardController.ServerTryBeginDrawCards`. Missing: two-card turn history, draw interception, private face-down attachment storage, three-crystal lifetime, and release into the correct player's hand.

### Sweet Escape

回合开始时，若自身前后2格内的地块[收费标价]之和大于等于2000（RiNG按当前收费乘数计入）：可打出此卡，依次进行以下操作：

1. [传送]至与“周边精选”、“商店街”或“东京外”对应颜色的、除地产商以外任意1个不属于你的未抵押格子。
2. 进行[结算]；若该格子可购买，必须购买。

此次[传送]视为你的[主要移动]。

#### Script allocation

- `SU10PlayEffect`: entire turn-start valuation gate, destination selection,
  teleport settlement, and mandatory purchase.

#### Implementation APIs

- Calls: movement-plan setters and `TileController.ServerTryResolveUnownedPropertyPurchase` with a valid quote. Missing: neighborhood price checks, legal color/destination choice, mandatory purchase orchestration, and main-movement entry.

### [三角初华]儿时玩伴的鼓励

用于确定移动步数的[投掷]前：可打出此卡，使本次移动以“小豆岛”为起点，并在此次移动后获得1个[火罐]。

#### Script allocation

- `SU11PlayEffect`: entire pre-roll origin replacement and post-movement fire pot.

#### Implementation APIs

- No complete API: replace the upcoming movement's origin with the authored island tile and grant a fire pot after completion. Define origin-trigger behavior; normal teleport cannot be assumed equivalent to changing the starting point.

### [纯田真奈]歌唱大赛5连冠

[反击]当你或你的格子即将受到来自你以外的效果影响时：打出此卡。此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。

若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽1张卡。

若没有人在此卡的效果期间打出[反击]卡，你抽1张卡。

#### Script allocation

- `SU12PlayEffect`: entire delegated-reaction window, substituted beneficiary,
  and success/no-response draw outcomes.
- Delegated cards retain their own effect scripts; this script coordinates
  the special response permissions and results.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` for the appropriate reward. Missing: delegated counter eligibility, substitution of self-referential targets, tracking successful protection, and determining whether any eligible counter was played.

### unnamed

将此卡置于自身场上，并为此卡添加2个[奇迹水晶]。

每当乐队技能需要移除[奇迹水晶]时：可改为移除此卡上的1个[奇迹水晶]。

此卡的[奇迹水晶]数量为0时，将此卡放入弃卡区。

#### Script allocation

- `SU13PlayEffect`: field placement and two-crystal initialization.
- `SU13FieldEffect`: band-crystal cost substitution and exhaustion discard.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for field placement/discard. Missing: two card crystals and a source-specific replacement for band-crystal consumption; consume the substitute only when the original cost is actually due.

### unnamed

[手]：

依次进行以下操作：

1. 从弃卡区或抽卡区选择1张“Sumimi”卡，加入手牌。
2. 将此卡置于自身场上。

[持续]：

（1）[拥有者]的手牌数量上限减少1。

（2）每当你使用[火罐]时：为此卡添加1个[奇迹水晶]。

（3）此卡拥有6个[奇迹水晶]时：将此卡返回手牌。

#### Script allocation

- `SU14HandEffect`: entire [手] search/retrieval and field placement.
- `SU14FieldEffect1`: [持续]（1）, hand-limit reduction.
- `SU14FieldEffect2`: [持续]（2）, fire-pot-spending crystals.
- `SU14FieldEffect3`: [持续]（3）, return to hand at six crystals.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCard` for selected Sumimi retrieval, field placement, and return to hand. Missing: private search/selection, hand-limit adjustment, fire-pot-spend events, and six-crystal threshold handling.
