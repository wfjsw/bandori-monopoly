# Poppin'Party

## Script allocation conventions

- Script identifiers below are proposed, not implemented classes.
- Each entry remains one effect source with separately registered effect
  components: BaseBandEffect for band effects, BaseCharacterAbility for
  character effects, and BaseCardEffect for card effects.
- Each numbered effect has its own script. Separate hand, special, and
  continuous/field blocks have separate scripts; numbering is local to its
  block. Explanatory parentheses and alternative choices are not effects.
- “依次进行以下操作” groups all following steps into one effect instance,
  executed in order. Step numbers do not define separate effects or scripts.
- If that effect contains a [共鸣] branch, offer one harmonize response
  opportunity before step 1 and complete the response before executing
  the steps. Do not reopen that opportunity within the same instance.
  Harmonize changes only the explicitly specified steps, not the whole effect.
- Band and character abilities use 自己 and place [自动] before [主动].
  Each category numbers its effects independently from （1）. References
  include both category and number. Choices inside automatic effects do
  not by themselves make those effects active.
- Each script owns the complete indicated clause, including eligibility,
  costs, choices, API calls, continuations, and source-specific cleanup.
  Multiple triggers within one numbered clause remain in that script.
- Non-money balances are player-owned authoritative resource records, not
  duplicated fields on effect scripts. Source-bound lifetime is separate from
  ownership and grant provenance; non-resource bookkeeping remains source state.
  Follow the accepted [RES-01 contract](../ARCHITECTURE.md#non-money-resource-contract--res-01);
  Resource value types, asset definitions, storage primitives and facade/prefab wiring
  are implemented; Arisa's lifecycle/activation consumer and permitted projections/UI
  remain Pending. Storage support alone does not implement this ability.
- Character entry starts with zero fire pots and the incoming cap. A written
  initial fire-pot amount is a separate game-start grant, not a replacement or
  re-entry grant. An explicit creating-effect grant is a separate exception.
  Character/band exit clears only its bound pools; player-direct fans and temporary
  grants survive. Field exit clears card-bound resources; ordinary re-entry uses
  authored defaults. Initial copies preserve zero or explicit creation grants.
- Resource costs preserve grant identity and explicit player choice. Validate the
  complete selected cost before consuming it; consume before usage-triggered
  payment or ability effects. Reject stale handles without redirecting them.
  Already-admitted effects retain their original context unless explicitly canceled;
  source exit neither refunds costs nor grants new activation permission.
- S04/S05 [board and player queries](../ARCHITECTURE.md#board-query-and-offboard-checkpoint--s04s05)
  are implemented; compilation/runtime checks remain Not Run. Supply ordered
  candidate IDs; board queries exclude missing/OffBoard positions and retain the
  first duplicate ID. Query results do not authorize effects or mutate movement.
  Potential paths are capped at 255 entries and are not executed-movement history.
- Gameplay OffBoard entry uses replacement relocation, not a storage write alone.
  It stops that player's replaced movement and suppresses new movement reactions,
  including MovementFinished. Already-open windows finish normally; each ability's
  CanRespondToEvent enforces its own OffBoard eligibility. Returning OnBoard needs
  an explicit destination and does not resume replaced movement.
- Card scripts explicitly gate their applicable zones. Moving a card does
  not create another source or automatically authorize every effect.
- Script labels are not runtime EffectIndex values. Register components in
  authored order; effect removal can change their runtime indexes.
- Setup-only rules and declarative clauses retain their own script boundary,
  but must not invent a reaction window or player prompt.

## Band Ability

### 星之鼓动

[资源]：

（1）[星星贴纸]：初始0。
（2）[奇迹水晶]：初始0。

[自动]：

（1）自己[经过]1、16、31或46号格子时：获得1个[星星贴纸]。

（2）自己无法获取[CiRCLE奖励]。

（3）游戏开始时，所有Poppin' Party角色视为同时拥有“星之鼓动山丘”，并适用以下规则：

- Poppin' Party角色可正常抵押“星之鼓动山丘”；
- 非Poppin' Party角色对“星之鼓动山丘”的[结算]改为[分摊][支付]所有未抵押“星之鼓动山丘”的Poppin' Party角色；
- “星之鼓动山丘”不可通过抵押后的双倍[支付]购买，只有全部Poppin' Party角色[破产]后才可被正常购买。

（4）自己不能通过[主要移动]的[结算]盖房。

[主动]：

（1）自己[经过]1、16、31或46号格子，可依次进行以下操作：

1. 使用2个[星星贴纸]，为此卡添加1个[奇迹水晶]；

2. 使用此卡的2个[奇迹水晶]，选择自己[抽卡]1张或[获得]2000资金。此步骤每回合限1次。

上述两个步骤均可选择不执行。

#### Script allocation

- `PPAutomaticBandEffect1`: [自动]（1）—qualifying-pass sticker reward.
- `PPAutomaticBandEffect2`: [自动]（2）—CiRCLE reward prohibition.
- `PPAutomaticBandEffect3`: entire [自动]（3）—shared initial ownership, mortgage rules,
  settlement payment sharing, and purchase restrictions.
- `PPAutomaticBandEffect4`: [自动]（4）—main-movement settlement construction prohibition.
- `PPActiveBandEffect1`: entire [主动]（1）—post-reward optional conversions
  in their written order; only the crystal-spending step is once per turn.

#### Implementation APIs

- Calls: `DrawCardController.ServerTryBeginDrawCards`, `PaymentController.ServerTryPrintMoney`, and `TileController.ServerTryAddPropertyOwner`. Missing: sticker/crystal storage, shared-property initialization, CiRCLE reward suppression, acquisition restrictions, and main-movement construction prohibition.

## Characters

### 户山香澄 — 非凡之星

[资源]：

（1）[火罐]：初始0，上限1。

[自动]：

（1）其他玩家在“星之鼓动山丘”上[结算]时：自己获得1个[火罐]。

[主动]：

（1）自己的运营阶段：可使用1个[火罐]，依次进行以下操作：

1. 立刻进入移动阶段，将本回合的[主要移动]改为[传送]到任意自己拥有的格子。
2. 可选择盖房。

#### Script allocation

- `PPKasumiAutomaticAbility1`: [自动]（1）—gain fire pots from other players' settlement
  at 星之鼓动山丘; apply the stated initial amount and cap.
- `PPKasumiActiveAbility1`: [主动]（1）—spend a fire pot, choose an owned destination,
  enter movement, replace main movement with teleport, and offer construction.

#### Implementation APIs

- Calls: `MovementPlanController.ServerSetMovementPlan` with `MovementPlan.CreateTeleport`; `TileController.ServerTryResolvePropertyDevelopment` for approved paid construction. Missing: fire-pot accounting, owned-tile choice, effect-driven movement-phase entry, and permission to construct at this timing.

### 市谷有咲 — 盆栽爱好者

[资源]：

（1）[火罐]：初始0，上限2。

[自动]：

（1）每次有其他玩家在“流星堂”抽取事件时：自己获得1个[火罐]。

（2）回合结束时，如果自己本回合的[主要移动][经过]“流星堂”，且[移动终点]不为“流星堂”：自己抽取1个视为在“流星堂”抽取的事件，并获得1个[火罐]。

[主动]：

（1）自己[经过]“流星堂”时：可使用2个[火罐]，为自己的乐队技能卡添加1个[奇迹水晶]。

#### Script allocation

- `PPArisaAutomaticAbility1`: [自动]（1）—gain fire pots from other players' 流星堂
  event draws; entry is zero with cap two, with no nonzero game-start grant.
- `PPArisaAutomaticAbility2`: [自动]（2）—check the main-movement path and destination,
  then perform the attributed turn-end event draw and fire-pot reward.
- `PPArisaActiveAbility1`: [主动]（1）—on passing 流星堂, spend two fire pots
  to add a crystal to the band source.

#### Implementation APIs

- Call: `NeutralActionController.ServerTryDrawAndResolveEventCard`. Missing: MeteorHall draw provenance, main-movement path history, turn-end scheduling, and fire-pot/crystal accounting; the draw API has no tile-origin parameter.
- RES-01: validate and consume both selected fire pots before granting the band
  crystal. A potential-path query cannot prove the earlier main movement passed
  MeteorHall; this ability's history and resource integration remain Pending.

### 花园多惠 — 花园警察，出警！

[资源]：

（1）[火罐]：初始2，上限4。

[自动]：

（1）每次自己领取[CiRCLE奖励]时：[投掷]3d20。如果结果对应的格子不是“星空齿科”，则在该格子放置1个[多惠标记]。自己[经过]有[多惠标记]的格子时，移除该格子上的所有[多惠标记]，并获得X个[火罐]；X为此次移除的格子状态[多惠标记]数量。

[主动]：

（1）其他玩家发动角色或乐队技能的[主动]效果，或使用手牌的[手]效果时：可使用4个[火罐]，依次进行以下操作：

1. 抵消该效果。
2. 自己[获得]500资金。
3. 被抵消效果的玩家[获得]2000资金。

#### Script allocation

- `PPTaeAutomaticAbility1`: entire [自动]（1）—both [多惠标记] placement after
  CiRCLE rewards and collection on passing, including fire-pot initialization/cap.
- [多惠标记] is a tile status, not a resource; preserve placement, clearing,
  and conversion to [火罐].
- `PPTaeActiveAbility1`: entire [主动]（1）—eligible interception, four-fire-pot cost,
  cancellation, and both money rewards.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` for 3d20 and `PaymentController.ServerTryPrintMoney` separately for each reward. Missing: [多惠标记] tile statuses, fire-pot accounting, and cancellation of another card or ability.

### 山吹沙绫 — 焕然一新的天空中

[资源]：

（1）[火罐]：初始3，上限5。

[主动]：

（1）其他玩家一次[消耗]或[支付]至少1000资金，且自己不拥有[saaya标记]时：可依次进行以下操作：

1. 那名玩家[获得]200资金。
2. 自己获得1层[saaya标记]和1个[火罐]。

（2）自己[消耗]或[支付]资金时：可使用5个[火罐]，此次资金变动减少5000（最低0）。

---

[火罐]：一种通用资源，初始数量、上限及使用方式由对应效果规定。

[消耗]：直接失去资金，等同于删钱。

[支付]：玩家之间资金转移的付款方用语。

[获得]：直接获得资金，等同于印钱。

[投掷]：按指定的骰子数量与面数掷骰；1d10表示1个10面骰。

[可购买格子]：非CiRCLE，CiRCLE 咖啡厅，江户川乐器店，流星堂，或[地产商]的格子。

[saaya标记]：移动[投掷]的结果减少[投掷]1d10的结果。如果计算后的结果小于等于0，则移动到下一个[可购买格子]。移动[投掷]后减少1层。

#### Script allocation

- Wording review: the draft moves the automatic clause into [saaya标记] and reduces its stack after the movement roll. The script/API notes below and the rulebook status row still describe the main text; reconcile marker timing after acceptance.

- `PPSaayaAutomaticAbility1`: [自动]（1）—marker consumption,
  movement-dice reduction, and fallback movement.
- `PPSaayaActiveAbility1`: [主动]（1）—qualifying payment response,
  other-player money reward, and own marker/fire-pot gain.
- `PPSaayaActiveAbility2`: [主动]（2）—five-fire-pot cost and the zero-floored
  reduction of the eligible money operation.
- Fire-pot initial amount/cap are character resource metadata shared by
  these scripts, not another independently activated effect.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney`, `DiceController.ServerTryBeginRoll` for the subtracted d10, and `PaymentController.ServerTryAddMoneyAmountDifference` at an additive stage. Missing: marker/fire-pot state, final movement-result adjustment, fallback destination, and zero-floor enforcement.

### 牛込里美 — 里美的决心

[资源]：

（1）[火罐]：初始1，上限3。

[自动]：

（1）自己在自己的回合中[经过]任意Live House颜色格子后，于该回合结束时获得1个[火罐]。

[主动]：

（1）自己的运营阶段：可使用3个[火罐]，依次进行以下操作：

1. 立刻进入移动阶段。
2. [传送]至任意与自己[绝对距离]最远的[地产商]，并进行[结算]。
3. 可选择盖房。

#### Script allocation

- `PPRimiAutomaticAbility1`: [自动]（1）—record qualifying Live House passes and grant
  the turn-end fire pot; apply the stated initial amount and cap.
- `PPRimiActiveAbility1`: [主动]（1）—spend fire pots, select a farthest developer,
  enter movement, teleport, settle, and offer construction.

#### Implementation APIs

- Calls: `MovementPlanController.ServerSetMovementPlan` with `MovementPlan.CreateTeleport`; `TileController.ServerTryResolvePropertyDevelopment` for eligible construction. Missing: pass history, fire-pot accounting, farthest-developer choice, and movement-phase/settlement orchestration.

## Cards

### Popipa

[手]：

依次进行以下操作：

1. 获得1000资金，并将1张“Pipopa”加入抽卡区。
2. 抽1张卡。
3. [移除]此卡。

#### Script allocation

- `PPPopipaHandEffect`: entire [手] block—money, Pipopa creation,
  card draw, and self-removal in the written order.

#### Implementation APIs

- Sequence: `PaymentController.ServerTryPrintMoney` (1000), `CardCreationController.ServerCreateCard` (Pipopa in Deck), `DrawCardController.ServerTryBeginDrawCards` (one), then `CardRemovalController.ServerRemoveCard`. Specify insertion position; preserve ordering and handle partial failure.

### [衍生]Pipopa

[手]：

依次进行以下操作：

1. 获得1000资金。
2. 将1张“Popipapapipopa”放置在自身场上。
3. [移除]此卡。

#### Script allocation

- `PPPipopaHandEffect`: entire [手] block—money, creation of
  Popipapapipopa on the user's field, and self-removal.

#### Implementation APIs

- Sequence: `PaymentController.ServerTryPrintMoney` (1000), `CardCreationController.ServerCreateCard` (Popipapapipopa on the user's Field), then `CardRemovalController.ServerRemoveCard`. Creation needs a registered definition and explicit player-field destination.

### [衍生]Popipapapipopa

[资源]：

（1）此卡的[奇迹水晶]：上限10。

[持续]：

（1）[拥有者]每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]。

（2）[消耗]或[支付]时：可使用此卡的多个[奇迹水晶]，每个使此次资金变动减少150，最低为0。

#### Script allocation

- `PPPopipapapipopaFieldEffect1`: [持续]（1）—qualifying passes
  add crystals to this card, subject to its cap.
- `PPPopipapapipopaFieldEffect2`: [持续]（2）—choose and spend
  this card's crystals to reduce an eligible money operation.

#### Implementation APIs

- Call: `PaymentController.ServerTryAddMoneyAmountDifference` during the appropriate additive stage. Missing: card-scoped crystals, pass-trigger accounting, expenditure choice, and zero-floor enforcement; negative money amounts otherwise have reversal semantics.

### Returns

[特]：

（1）如果此卡被加入初始卡组则卡组卡数添加8。

（2）游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得1个其他存活玩家的乐队技能卡。

[持续]：

（1）无效[拥有者]Poppin' Party乐队卡的[自动]（4）效果。

（2）[拥有者]每回合开始时选择1个其他存活玩家的乐队技能卡，如果和当前因此卡获得的乐队技能卡不一样则替换并移除上面的所有[奇迹水晶]。

（3）使用因此卡获得的乐队技能卡的[主动]效果时需要使用1个[星星贴纸]。

（4）[拥有者]的通用卡的[手]效果全部生效后获得1个[星星贴纸]。

#### Script allocation

- `PPReturnsSpecialEffect1`: [特]（1）—initial-deck size modifier;
  requires setup/deck-validation integration, not a normal play prompt.
- `PPReturnsSpecialEffect2`: [特]（2）—initial field placement
  and acquisition of another living player's band source.
- `PPReturnsFieldEffect1`: [持续]（1）—suppress only the owner's
  Poppin'Party band [自动]（4）, not the entire band source.
- `PPReturnsFieldEffect2`: [持续]（2）—turn-start band choice,
  replacement when different, and the stated crystal cleanup.
- `PPReturnsFieldEffect3`: [持续]（3）—additional sticker cost
  for the acquired band's active effects.
- `PPReturnsFieldEffect4`: [持续]（4）—sticker reward after
  the owner's general-card hand effect fully succeeds.
- The acquired-band reference is shared source-associated state used by
  the relevant scripts; it is not independently copied by each script.
- RES-01: replacement clears only the outgoing acquired source's bound pools,
  not unrelated player pools. Initial copied-source placement starts at zero
  unless explicitly granted; later entry follows the applicable authored defaults.
  Include the extra sticker in the full active-ability cost before consumption.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for initial field placement. Missing: deck-size/setup modifiers, temporary band-ability copying, selective rule suppression, copied-source cleanup, sticker costs, and successful general-card-use tracking.

### Tomorrow's Door

（1）此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”。

（2）将此卡放置在“流星堂”上，[使用者]每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中的下一个，若此卡已位于“大阪中之岛公园”，则将此卡放置在此卡使用者的游玩区域。

（3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外[支付][拥有者]“星之鼓动山丘”上的房屋数量×100的资金。

#### Script allocation

- `PPTomorrowsDoorEffect1`: （1）—owns the declared waypoint sequence
  for this card. This data-only clause creates no activation prompt.
- `PPTomorrowsDoorEffect2`: entire （2）—initial placement, pass-triggered
  progression through that sequence, and final play-area placement.
- `PPTomorrowsDoorEffect3`: entire （3）—while in the stated play area,
  apply the additional settlement payment with its house-count formula.
- Effect 2 reads the sequence declared by effect 1; do not maintain
  separate conflicting copies. The existing play-area ambiguity remains.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for tile/player field placement and `PaymentController.ServerTryPlayerPayPlayer` for the additional charge. Missing: ordered waypoint state, pass-trigger tracking, and confirmation of the intended meaning of the final play area.

### Bang Dream!

[手]：

依次进行以下操作：

1. 为[使用者]的乐队技能卡添加1个[奇迹水晶]。
2. [传送]至任意[使用者]拥有的格子，且可选择盖房。

#### Script allocation

- `PPBangDreamHandEffect`: entire [手] block—add the band crystal,
  select an owned tile, teleport, and offer construction.

#### Implementation APIs

- Calls: `PlayerRelocationController.ServerTryTeleportPlayer` and `TileController.ServerTryResolvePropertyDevelopment`. Missing: band-crystal accounting and owned-tile/construction choice. Teleport completion does not itself authorize construction or perform tile settlement.

### STAR BEAT!

[STAR BEAT!标记]：下次[结算]后，可选择在[绝对距离]5格以内自己拥有的格子上进行1次盖房。本次盖房机会处理完毕后减少1层；放弃、无合法目标或未实际盖房也减少，不保留该次机会。此状态不是价格减免。

[手]：[使用者]获得1层[STAR BEAT!标记]，然后进入移动阶段并选择以下操作之一：

1. 获得2个[星星贴纸]，本回合的[主要移动]改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并[结算]；

2. 本回合的[主要移动]改为移动[投掷](2×“资金数包含5的玩家数量+1”)d10的结果所对应的格数并结算。

#### Script allocation

- `PPStarBeatHandEffect`: entire [手] block—grant the delayed construction
  status, enter movement, and execute the selected alternative.
  The numbered choices 1. and 2. are branches of this effect, not
  separately registered card effects.
- The granted status needs its own status-source behavior for the later
  settlement/construction opportunity and stack consumption. That behavior
  must not depend on this card remaining in hand or on the field.

#### Implementation APIs

- Call: `MovementPlanController.ServerSetMovementPlan` with a teleport or dice plan. Missing: sticker accounting, branch choice, effect-driven movement entry, and the delayed construction entitlement; a registered status can store stacks but needs authored behavior.

### 抓到了

选择你前方的1名玩家，移动到那名玩家所在的格子。此次移动视为本回合的[主要移动]，且可选择盖房。

#### Script allocation

- `PPCaughtYouPlayEffect`: entire unnumbered card text—select a player
  ahead, perform the main movement, and offer construction.
  The text does not explicitly label a hand/field block.

#### Implementation APIs

- Call: `MovementPlanController.ServerSetMovementPlan` with a computed fixed-step plan. Missing: legal forward-player choice, main-movement entry, and optional construction orchestration; teleport is not equivalent to traversing the intervening tiles.

### 迷宫般的仓库

位于“流星堂”前后5格内时：可打出此卡，[投掷]1d10。从“流星堂”开始移动，直到[经过]此次[投掷]结果对应数量的无主可购买地。此次移动视为你的[主要移动]。

本回合购买格子不[消耗]资金；若购买，拆除该格子上的所有房屋。

若[经过]“流星堂”，则[强制停下]，并[消耗]6000资金。

#### Script allocation

- `PPLabyrinthWarehousePlayEffect`: entire unnumbered card text—play
  eligibility, die roll, origin/counting changes, main movement,
  conditional free purchase/demolition, and 流星堂 stop/payment.
- Later movement and purchase behavior uses scoped continuations or
  modifiers belonging to this activation, not extra invented card effects.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (1d10) and `PaymentController.ServerTryDeleteMoney` (6000). Missing: silent origin replacement, movement counting only qualifying properties, force-stop, free-purchase authorization, and house demolition.

### 献给远方的你

[手]：

依次进行以下操作：

1. [传送]至任意与[使用者][绝对距离]最远的玩家所在的格子，并进行[结算]。
2. 可选择任意与[使用者][绝对距离]最远的、[使用者]拥有且可盖房的格子，加盖房屋。

例如：[使用者]在11号格子，1名玩家在1号格子，两者的[绝对距离]为10。

#### Script allocation

- `PPToYouFarAwayHandEffect`: entire [手] block—farthest-player choice,
  teleport, settlement, then optional farthest-owned-property construction.
  These are dependent steps of one effect, not separate scripts.

#### Implementation APIs

- Sequence: `PlayerRelocationController.ServerTryTeleportPlayer`, then `TileResolutionController.ServerTryBeginResolution`; use `TileController.ServerTryResolvePropertyDevelopment` for eligible paid construction. Missing: farthest-player/property selection and special remote-construction permission.
- S05 supplies `MatchBoardStateData.GetFarthestPlayersSnapshot` with
  `BoardDistanceMode.AbsoluteNumber`, retaining ties, and `GetBoardDistance`
  for filtering owned/developable tiles. Authenticated choices and the construction
  permission above remain Pending.

### 仓库里的Random Star

（1）依次进行以下操作：

1. 将此卡放置在自身场上，并将弃卡区和手牌洗入卡组。
2. 将1张“拍卖撤下来了”放置在卡组底端。

（2）[经过]“流星堂”时：可使用2个[星星贴纸]，在“流星堂”[强制停下]并[结算]。

#### Script allocation

- `PPWarehouseRandomStarEffect1`: entire （1）—self-placement,
  discard/hand recycling, and bottom-deck creation of 拍卖撤下来了.
- `PPWarehouseRandomStarEffect2`: entire （2）—the placed card's
  流星堂 pass response, sticker cost, forced stop, and settlement.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `DeckController.ServerRecycleDiscardIntoDeck`, `DeckController.ServerReturnHandToDeckAndShuffle`, and `CardCreationController.ServerCreateCard` at the final deck index. Missing: sticker costs and force-stop/settlement integration.

### [衍生]拍卖撤下来了

[特]：

此卡加入手牌时将此卡放置在自身场上，每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）。

[持续]：此卡[拥有者]的[火罐]上限增加1且不受任何其他效果影响。

#### Script allocation

- `PPAuctionWithdrawnSpecialEffect`: entire [特] block—hand-entry
  placement and turn-start removal/payment. Both trigger timings remain
  in this unnumbered special block's script.
- `PPAuctionWithdrawnFieldEffect`: entire [持续] block—fire-pot
  cap increase and the stated immunity. Preserve the original scope;
  this allocation does not resolve ambiguous wording.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `CardRemovalController.ServerRemoveCard`, and `PaymentController.ServerTryDeleteMoney` (540). Missing: hand-entry interception, fire-pot-cap modification, immunity enforcement, and clarification of whether payment requires finding the other card.

### 向着未来的路标

将本回合的[主要移动]设为移动60格，不触发[结算]。回合结束时，[消耗]1000资金。

#### Script allocation

- `PPFutureSignpostPlayEffect`: entire unnumbered card text—60-space
  main movement without settlement and the scheduled turn-end loss.
  The delayed loss belongs to this activation, not another card effect.

#### Implementation APIs

- Calls: `MovementPlanController.ServerSetMovementPlan` with `MovementPlan.CreateFixedStep(60)`, `ServerSetResolveTileAfterMovement(false)`, and delayed `PaymentController.ServerTryDeleteMoney` (1000). Missing: main-movement entry and turn-end scheduling.

### [户山香澄]大家我都喜欢哦

[手]：

将此卡放置在“星之鼓动山丘”上。

[持续]：

其他玩家[经过]此卡所在格子，且[移动终点]不为该格子时，依次进行以下操作：

1. 那名玩家在此卡所在格子[强制停下]。
2. 将此卡放入[使用者]弃卡区，并为[使用者]的乐队技能卡添加1个[奇迹水晶]。

那名玩家此次[结算]若[支付]地租，地租金额为原本的一半。

#### Script allocation

- `PPEveryoneILoveYouHandEffect`: [手]—place this card on 星之鼓动山丘.
- `PPEveryoneILoveYouFieldEffect`: entire [持续] block—eligible pass
  interception, forced stop, discard, band crystal, and the correlated
  rent reduction. The reduction must survive the stated discard long
  enough to affect that settlement, without enabling later activations.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for tile placement/discard and `PaymentController.ServerTryMultiplyMoneyAmount` (1/2) for the correlated rent. Missing: force-stop, original-destination checks, and band-crystal accounting.

### [市谷有咲]等等等一下

[手]：

将此卡放置在[使用者]的[场地]。

[持续]：

（1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。

（2）事件触发时，若此卡至少拥有3个[奇迹水晶]：使用此卡的3个[奇迹水晶]，使那个事件在[拥有者]回合开始时结算。此后，额外抽取1个视为其他玩家在“流星堂”抽取的事件。

#### Script allocation

- `PPWaitWaitHandEffect`: [手]—place this card on the user's field.
- `PPWaitWaitFieldEffect1`: [持续]（1）—gain crystals from eligible
  event resolutions, excluding events caused by this card.
- `PPWaitWaitFieldEffect2`: entire [持续]（2）—check/spend crystals,
  suspend and later resolve the event, then perform the attributed extra draw.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `NeutralActionController.ServerTryDrawAndResolveEventCard`. Missing: crystal accounting, neutral-event suspension/resumption, self-origin exclusion, and an attributed MeteorHall draw on another player's behalf.

### [花园多惠]寻找更好的声音

[传送]到“江户川乐器店”，并获得1个[火罐]。

#### Script allocation

- `PPBetterSoundPlayEffect`: entire unnumbered card text—teleport
  to 江户川乐器店 and gain a fire pot; no additional settlement is implied.

#### Implementation APIs

- Call: `PlayerRelocationController.ServerTryTeleportPlayer` to the authored instrument-shop tile. Missing: fire-pot accounting. Do not add tile settlement unless the applicable rules require it.

### [山吹沙绫]总有一天要给这片天空命名

（1）将此卡放置在山吹面包房。

（2）[使用者][经过]此卡后，在回合结束后依次进行以下操作：

1. 获得1个[火罐]。
2. [投掷]3d20，将此卡放置在此次[投掷]结果对应的格子上。

#### Script allocation

- `PPNameThisSkyEffect1`: （1）—initial placement at 山吹面包房.
- `PPNameThisSkyEffect2`: entire （2）—track the user's qualifying pass,
  award the delayed fire pot, roll 3d20, and relocate this card.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for tile placement and `DiceController.ServerTryBeginRoll` (3d20). Missing: fire-pot accounting and once-per-qualified-turn pass history; map displayed tile numbers to runtime indexes explicitly.

### [牛込里美]我的心就像巧克力螺

[手]：

立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子[绝对距离]1到4格或以内的任何格子并[结算]，期间[不可阻挡]。

#### Script allocation

- `PPChocolateCornetHandEffect`: entire [手] block—destination choice,
  movement-phase entry, replacement main movement, unstoppable traversal,
  and settlement. Preserve the written distance restriction.

#### Implementation APIs

- Building blocks: `MatchBoardStateData.GetBoardDistance` with
  `BoardDistanceMode.AbsoluteNumber`, independent `GetPotentialMovementPathSnapshot`,
  and movement-plan setters. Missing: authenticated destination/direction choice,
  movement-phase entry, and unstoppable movement. A queried path is not an
  executable movement plan; preserve step movement rather than substituting teleport.
