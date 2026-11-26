# Pastel＊Palettes

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
PA-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.
Copied character effects use the corresponding effect implementation with
separate recipient state; do not copy the entire character source.
Under RES-01, P✽P fans are player-direct resources: character/band replacement
or card exit does not clear them. Having fans does not retain an exited ability's
activation permission. Resource APIs remain Pending.

## Band Ability

### 与偶像一起

[资源]：

（1）此卡的[奇迹水晶]：上限10。

[自动]：

（1）游戏开始时：所有非Pastel✽Palettes角色获得1个反面[P✽P粉丝]。场上有多张Pastel✽Palettes乐队卡时，此效果不重复发动。

（2）自己因任意原因受到“将X个反面[P✽P粉丝]变正”的效果，且X大于自己拥有的反面[P✽P粉丝]数量时：为此卡添加等于溢出数量的[奇迹水晶]。

（3）自己[经过]CiRCLE时不获得[CiRCLE奖励]。

（4）自己的回合开始时：为此卡添加1个[奇迹水晶]。

[主动]：

（1）自己的回合开始时：可选择使用此卡的5个[奇迹水晶]，然后自己[抽卡]1张。

#### Script allocation

- `PA01AutomaticBandEffect1`: [自动]（1）, non-Pastel fan setup with duplicate suppression.
- `PA01AutomaticBandEffect2`: [自动]（2）, mandatory conversion of excess fan flips to capped crystals.
- `PA01AutomaticBandEffect3`: [自动]（3）, CiRCLE reward suppression.
- `PA01AutomaticBandEffect4`: [自动]（4）, crystal gain at the start of the player's own turn.
- `PA01ActiveBandEffect1`: [主动]（1）, optional five-crystal spending and draw at the start of the player's own turn.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` for the crystal exchange. Missing: once-per-match fan initialization, face-up/down fan conversion, overflow crystals, turn-start recharge, and CiRCLE reward suppression.

## Characters

### 丸山彩 — With~

[自动]：

（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得丸山彩的[主动]（1）技能。

[主动]：

（1）自己每次需要[支付]资金时：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：

1. 此次[支付]的分摊前金额减少Y×100（最低0）；

2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。

#### Script allocation

- `PA02AutomaticAbility1`: [自动]（1）, initial fans and grants of [主动]（1）.
- `PA02ActiveAbility1`: entire [主动]（1）, fan cost, payment reduction, and fan redistribution.

#### Implementation APIs

- Call: `PaymentController.ServerTryAddMoneyAmountDifference` at the pre-split payment additive stage. Missing: fan initialization/conversion, shared ability grants, expenditure selection, cross-player fan allocation, and zero-floor enforcement.

### 若宫伊芙 — 天下统一

[自动]：

（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙的[主动]（1）技能。

[主动]：

（1）自己因任意原因[投掷]前：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：

1. 为此次[投掷]额外增加Y个d4骰子；

2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。

#### Script allocation

- `PA03AutomaticAbility1`: [自动]（1）, initial fans and grants of [主动]（1）.
- `PA03ActiveAbility1`: entire [主动]（1）, fan cost, additional dice, and fan redistribution.

#### Implementation APIs

- Movement-only building block: `MovementPlanController.ServerModifyMovementDice` during BeforeDiceRoll. Missing: general-roll dice modification, fan costs/conversion, and shared ability grants; this movement API does not cover every roll.

### 白鹭千圣 — 保持坦率的你

[自动]：

（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得白鹭千圣的[主动]（1）技能。

[主动]：

（1）自己每次[收取]资金时：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：

1. 此次[收取]的分摊前金额增加Y×100；

2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。

#### Script allocation

- `PA04AutomaticAbility1`: [自动]（1）, initial fans and grants of [主动]（1）.
- `PA04ActiveAbility1`: entire [主动]（1）, fan cost, receipt increase, and fan redistribution.

#### Implementation APIs

- Call: `PaymentController.ServerTryAddMoneyAmountDifference` at the appropriate pre-split receipt stage. Missing: shared ability grants, fan initialization/conversion, chosen fan quantity, and validation of which receipt operations qualify.

### 大和麻弥 — 朝阳照耀的片刻

[自动]：

（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得大和麻弥的[主动]（1）技能。

[主动]：

（1）自己[抽卡]时：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：

1. 此次[抽卡]改为观看自己卡组顶端Y+1张卡（卡组数量不足Y+1则观看全部卡组），然后选择其中1张卡加入自己手牌（不公开，此次加入手牌视为[抽卡]动作），将剩余观看的卡洗回自己卡组；

2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。

#### Script allocation

- `PA05AutomaticAbility1`: [自动]（1）, initial fans and grants of [主动]（1）.
- `PA05ActiveAbility1`: entire [主动]（1）, fan cost, private deck inspection/selection,
  draw replacement, shuffle, and fan redistribution.

#### Implementation APIs

- Building blocks: `CardZoneController.ServerTryMoveCard` and `DeckController.ServerShuffleDeck`. Missing: owner-private top-card inspection/selection, draw replacement preserving draw events, fan conversion, and shared ability grants; direct zone movement is not a draw.

### 冰川日菜 — 日菜抽中的大奖

[自动]：

（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得冰川日菜的[自动]（2）技能。

（2）每回合开始时：自己必须[投掷]1d4，根据最终[投掷]结果获得以下对应角色的[主动]（1）技能，持续至下回合开始前：

- 结果为1：丸山彩；
- 结果为2：白鹭千圣；
- 结果为3：若宫伊芙；
- 结果为4：大和麻弥。

#### Script allocation

- `PA06AutomaticAbility1`: [自动]（1）, initial fans and grants of [自动]（2）.
- `PA06AutomaticAbility2`: entire [自动]（2）, mandatory roll and temporary grant of the selected character's [主动]（1）.

#### Implementation APIs

- Call: `DiceController.ServerTryBeginRoll` for mandatory turn-start 1d4. Missing: temporary character-effect grants/replacement, expiration before the next turn, and fan initialization; preserve the selected ability's original resource and targeting contracts.

## Cards

### 再次闪耀

[特]：

此卡不受除拥有此卡的玩家以外的玩家的效果影响。

[手]：

将此卡放置在[使用者]的[场地]。

[持续]：

（1）[反击][拥有者][消耗]或[支付]并当前资金不够时：依次进行以下操作：

1. 记录1个此卡未记录、且[拥有者]拥有同色地契的颜色。
2. [拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。

（2）[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]。

#### Script allocation

- `PA07SpecialEffect`: [特], protection from other players' effects.
- `PA07HandEffect`: [手], field placement.
- `PA07FieldEffect1`: [持续]（1）, insufficient-money response and color/rewards.
- `PA07FieldEffect2`: [持续]（2）, resonance color recording and fan reward.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `PaymentController.ServerTryPrintMoney`. Missing: recorded-color state, fan grants, resonance dispatch, other-player immunity, and safe integration with an already pending insufficient-funds payment.

### 同一个梦想

[手]：

依次进行以下操作：

1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；

2. 将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金；

3. 将自己的所有反面[P✽P粉丝]变正，如果[共鸣]则此效果对所有Pastel✽Palettes角色生效。

#### Script allocation

- `PA08HandEffect`: entire [手] block, all three ordered steps and resonance scope.

#### Implementation APIs

- Call: `PaymentController.ServerTryPrintMoney` using the captured count of flipped fans. Missing: ordered crystal/fan mutations, resonance detection, and the all-Pastel target expansion; do not recalculate the award after restoring fan faces.

Accepted DEC-11 contract (2026-10-18): all three steps share one effect instance.
Request and complete harmonize before step 1, retaining the same hasBeenHarmonized
guard through all three steps. Only step 3 expands to other Pastel characters when
harmonized; steps 1 and 2 retain their ordinary scope. Preserve the existing card text.
Follow the [shared harmonize lifecycle](../ARCHITECTURE.md#harmonize-effect-instance-lifetime);
implementation/API alignment remains Pending and runtime checks Not Run.

### 初次演出事故

[手]：

依次进行以下操作：

1. 将此卡放置在[使用者]的[场地]。
2. 将1张“重叠的声音”加入抽卡区，并洗切。
3. 抽1张卡。

[持续]：

[拥有者]不可使用丸山彩、若宫伊芙、白鹭千圣或大和麻弥的[主动]（1）技能，也不可使用冰川日菜的[自动]（2）技能。

#### Script allocation

- `PA09HandEffect`: entire [手] placement, creation/shuffle, and draw.
- `PA09FieldEffect`: [持续], prohibition of Aya/Eve/Chisato/Maya [主动]（1） and Hina [自动]（2）.

#### Implementation APIs

- Sequence: `CardZoneController.ServerTryMoveCardToEnd`, `CardCreationController.ServerCreateCard`, `DeckController.ServerShuffleDeck`, then `DrawCardController.ServerTryBeginDrawCards`. Missing: a field-scoped prohibition on all listed character secondary abilities.

### [衍生]重叠的声音

[特]：

此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上的“初次演出事故”（如果有）。

[手]：

依次进行以下操作：

1. [移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；

2. 进入移动阶段并将本回合的[主要移动]改为[传送]到“Bandori车站”且不[结算]；

3. 回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。

#### Script allocation

- `PA10SpecialEffect`: [特], discard-entry removal of 初次演出事故.
- `PA10HandEffect`: entire [手] block, including its delayed rewards/card creation.

#### Implementation APIs

- Calls: `CardRemovalController.ServerRemoveCard`, `MovementPlanController.ServerSetMovementPlan` with a non-settling teleport plan, `StatusController.ServerModifyStatusStacks`, and `CardCreationController.ServerCreateCard`. Missing: discard-entry trigger, phase transition, turn-end scheduling, and fan grant.

### [衍生]明天见

[特]：

（1）只有[使用者]的正面[P✽P粉丝]数量小于等于反面[P✽P粉丝]数量才可使用。

（2）[共鸣]无视此卡的[特]效果（1）。

[手]：

将X个反面[P✽P粉丝]变正；X为自己拥有的[P✽P粉丝]数量。

#### Script allocation

- `PA11SpecialEffect1`: [特]（1）, fan-count use restriction.
- `PA11SpecialEffect2`: [特]（2）, resonance exception to that restriction.
- `PA11HandEffect`: entire [手] fan-flip operation.

#### Implementation APIs

- No complete fan API: validate the face-count eligibility or resonance override, then request conversion of the stated total fan count. Route excess conversion through the band ability's overflow rule.

### 不要背负期待

[特]：

此卡不受任何其他效果影响。

[手]：

依次进行以下操作：

1. 将此卡放置在[使用者]的[场地]。
2. 将1张“共鸣”加入卡组。
3. 抽1张卡。

[持续]：

（1）以下效果对[拥有者]持续生效：非回合开始时进行的[投掷]结果减少2；若该[投掷]用于确定移动步数，最终结果最低为0，结果为0时此次移动不[结算]。手牌数量上限减少1。

（2）以下效果此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次：[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）。

#### Script allocation

- `PA12SpecialEffect`: [特], immunity.
- `PA12HandEffect`: entire [手] placement, card creation, and draw.
- `PA12FieldEffect1`: entire [持续]（1）, dice and hand-limit modifiers.
- `PA12FieldEffect2`: entire [持续]（2）, placement/recycle-triggered money modifiers.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `CardCreationController.ServerCreateCard`, `DrawCardController.ServerTryBeginDrawCards`, and `PaymentController.ServerTryAddMoneyAmountDifference`. Missing: immunity, general-roll adjustment, zero-move settlement suppression, hand-limit changes, and recycle-triggered two-use modifiers.

### [衍生]共鸣

[特]：

拥有此卡的玩家的卡可发动[共鸣]时，依次进行以下操作：

1. 将此卡放入弃卡区。
2. 发动那张卡的[共鸣]效果。
3. 为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2个[奇迹水晶]。

#### Script allocation

- `PA13SpecialEffect`: entire [特] block, discard, activation of the other
  card's resonance behavior, and band crystals. The receiving card retains
  its own effect script and identity.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` to Discard. Missing: resonance eligibility/dispatch and band-crystal accounting; discarding this card must be coupled to the selected resonance effect and its resource grant.

### 有你与我在这里共度

[手]：

X为5。若[使用者]拥有的正面[P✽P粉丝]数量少于反面数量，X额外增加“反面数量减去正面数量”的一半，向上取整。若[共鸣]，X减少5。

将X个反面[P✽P粉丝]变正；若[共鸣]，另抽1张卡。

#### Script allocation

- `PA14HandEffect`: entire [手] block, X calculation, fan flips,
  and the resonance adjustment/draw.

#### Implementation APIs

- Call: `DrawCardController.ServerTryBeginDrawCards` for the resonance branch. Missing: fan-state queries/conversion, resonance choice, and overflow handling; capture the original face-count difference before changing fans.

### 梦在前方，结彩当下

[资源]：

（1）此卡的[奇迹水晶]：上限5。

[特]：

游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1。

[持续]：

（1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]，此卡每获得1个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）。

（2）[拥有者]不可盖房，且手牌数量上限减少1。

（3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。

（4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]。

#### Script allocation

- `PA15SpecialEffect`: [特], initial field placement and initial-hand reduction.
- `PA15FieldEffect1`: [持续]（1）, draw crystals, cap, and excess counter X.
- `PA15FieldEffect2`: [持续]（2）, construction prohibition and hand limit.
- `PA15FieldEffect3`: [持续]（3）, additional property-settlement payment.
- `PA15FieldEffect4`: entire [持续]（4）, resonance reaction and remote purchase.
- RES-01: clamp crystal gains at five and add the reported overflow to X;
  X remains effect bookkeeping, not a duplicate crystal balance. Cap reduction,
  expiry, and cleanup are not gain overflow. Consume the selected three crystals
  before the purchase effect. Field exit clears this card's bound resources,
  not player-direct fans. Resource/cost integration remains Pending.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryPlayerPayPlayer`, and `TileController.ServerTryResolveUnownedPropertyPurchase` with a valid quote. Missing: setup/hand-limit changes, crystal overflow X, construction prohibition, resonance, and remote-property selection.

### TITLE IDOL

[手]：

依次进行以下操作：

1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；

2. 为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。

#### Script allocation

- `PA16HandEffect`: entire [手] block; both crystal-distribution steps
  and the resonance quantity change remain together.

#### Implementation APIs

- No complete crystal API: enumerate the user's eligible band/card sources and add the specified crystals with each source's cap/overflow rules. Resonance and machine-readable crystal eligibility must be authored; do not search display text at runtime.

### 可爱又强壮的花朵

[手]：

[指定][使用者]拥有的1个格子，依次进行以下操作：

1. 将此卡放置在被[指定]的格子上。
2. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]。

[持续]：

[使用者][经过]此卡所在格子时依次进行以下操作：

1. [强制停下]，此次移动变为[结算]；

2. 如果[共鸣]则获得1500资金；

3. 此卡放入[使用者]弃卡区。

#### Script allocation

- `PA17HandEffect`: entire [手] targeting, placement, and band crystals.
- `PA17FieldEffect`: entire [持续] pass response, all three ordered steps.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for tile placement/discard and `PaymentController.ServerTryPrintMoney` (1500). Missing: owned-tile choice, band crystals, resonance, and force-stop with exactly one required settlement.

### 找回珍妮弗

[手]：

将此卡放置在除[使用者]以外的一名玩家的[场地]。

[持续]：

[拥有者][经过]“偶像经纪公司”时依次进行以下操作：

1. [使用者][支付][拥有者]400资金；

2. [使用者]获得1个正面的[P✽P粉丝]，[拥有者]将一个[P✽P粉丝]变为正面；

3. 此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡。

#### Script allocation

- `PA18HandEffect`: [手], placement on another player's field.
- `PA18FieldEffect`: entire [持续] pass response, payment, fans, removal,
  and replacement-card creation. Preserve user/owner distinctions.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryPlayerPayPlayer`, `CardRemovalController.ServerRemoveCard`, and `CardCreationController.ServerCreateCard`. Missing: fan conversion and owner-versus-field-host clarification; placing a card on another player's field retains ownership.

### [衍生]魔法战队Pastel✽Ranger

[特]：

仅在你的[绝对距离]30-“粉丝数量”格内有你拥有房的格子时可使用。

[手]：

根据场上你拥有的卡数量并依次进行以下操作，如果[共鸣]则视为数量加1：

1. 数量至少为1则[获得]500资金；

2. 数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]；

3. 数量至少为4则使用Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡；

4. 数量至少为5则先[消耗]2000资金，再获得1层[盖房免除2000]；

5. 数量至少为6则抽1张卡。

#### Script allocation

- `PA19SpecialEffect`: [特], distance/property eligibility.
- `PA19HandEffect`: entire [手] block, adjusted count and all threshold steps.
- Granted construction-discount status behavior belongs to its status source,
  not to additional hand-effect scripts. Step 4's debit occurs before the grant,
  not inside the status behavior; partial or canceled payment still grants it.
- [盖房免除2000] follows the [rulebook's construction rules](../RULEBOOK.md#角色卡牌与格子状态):
  overflow is allowed; apply one layer per distinct discount amount, fixed
  reductions before multipliers, and remove applied layers only after actual
  construction completes.

#### Implementation APIs

- Calls: `PaymentController.ServerTryPrintMoney`, `DrawCardController.ServerTryBeginDrawCards`, and `StatusController.ServerModifyStatusStacks` for an authored development-credit status. Missing: crystal/fan accounting, resonance, threshold sequencing, and a development discount supporting the specified overflow.

### 练习生解密指南

[手]：

依次进行以下操作：

1. 为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]，并将此卡放置在[使用者]的[场地]。
2. 在此卡上放置“粉丝数量”÷3个[奇迹水晶]。
3. 公开[使用者]的抽卡区，根据公开卡中的颜色数量添加1个单色或双色标记；如果[共鸣]，则[消耗]500资金并添加任意2个标记。

[持续]：

（1）手牌数量上限减少1。

（2）回合结束时添加1个[奇迹水晶]。

（3）此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行以下操作随后进入弃卡区：

1. 每个单色效果为获得1层[盖房免除1000]；

2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正。

#### Script allocation

- `PA20HandEffect`: entire [手] setup, crystals, reveal, markers, and resonance.
- `PA20FieldEffect1`: [持续]（1）, hand-limit reduction.
- `PA20FieldEffect2`: [持续]（2）, turn-end crystal.
- `PA20FieldEffect3`: entire [持续]（3）, threshold, marker outcomes, and discard.
  Its two numbered outcome steps are not separate card effects.
- [盖房免除1000] follows the [rulebook's construction rules](../RULEBOOK.md#角色卡牌与格子状态):
  overflow is allowed; apply one layer per distinct discount amount, fixed
  reductions before multipliers, and remove applied layers only after actual
  construction completes.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, `PaymentController.ServerTryDeleteMoney`, and `StatusController.ServerModifyStatusStacks` for authored credits. Missing: authorized deck reveal, color-marker selection, crystal/fan accounting, hand-limit modification, resonance, and crystal-division rounding.

### [丸山彩]憧憬的前方

[手]：

依次进行以下操作：

1. 将此卡放置在[使用者]的[场地]。
2. 将弃卡区中的1张卡加入手牌。

[持续]：

（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。

（2）[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。

#### Script allocation

- `PA21HandEffect`: entire [手] placement and card retrieval.
- `PA21FieldEffect1`: [持续]（1）, wealth/fan-dependent money reduction.
- `PA21FieldEffect2`: [持续]（2）, resonance money-reduction reaction.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` for field placement/discard retrieval and `PaymentController.ServerTryAddMoneyAmountDifference`. Missing: private discard choice, live minimum-money/fan checks, resonance, and zero-floor enforcement.

### [大和麻弥]可能性为∞

[特]：

观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场地]（此效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。

[持续]：

（1）[拥有者]进行抽卡动作后，根据此卡的[奇迹水晶]数量进行对应操作：

- 数量小于4：为此卡添加1个[奇迹水晶]。
- 否则：移除此卡的[奇迹水晶]，并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。

（2）[共鸣]交换[拥有者]的抽卡区和弃卡区。

#### Script allocation

- `PA22SpecialEffect`: entire [特] inspection interception, reveal, and placement.
- `PA22FieldEffect1`: [持续]（1）, draw-triggered crystal accumulation/conversion.
- `PA22FieldEffect2`: [持续]（2）, resonance deck/discard exchange.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for field placement. Missing: priority interception during private deck inspection, controlled public reveal, crystal transfers, resonance, and a zone-swap operation preserving defined order and trigger semantics.

### [白鹭千圣]微笑的铁假面

[手]：

依次进行以下操作：

1. 将此卡放置在[使用者]的[场地]。
2. 其他玩家[分摊][支付][使用者]2000资金。

[持续]：

（1）[拥有者]的弃卡区洗入抽卡区时其他玩家[分摊][支付][拥有者]500资金。

（2）[共鸣]其他玩家[分摊][支付][拥有者]1500资金。

#### Script allocation

- `PA23HandEffect`: entire [手] placement and shared payment.
- `PA23FieldEffect1`: [持续]（1）, recycle-triggered shared payment.
- `PA23FieldEffect2`: [持续]（2）, resonance shared payment.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and `PaymentController.ServerTryPlayerPayPlayer` with total 2000, 500, or 1500 shared across other payers. Missing: discard-recycle trigger attribution and resonance dispatch.

### [冰川日菜]会发出怎样的声音呢？

[特]：

（1）此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡。

（2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。

#### Script allocation

- `PA24SpecialEffect1`: [特]（1）, eligible exclusive-card substitution.
- `PA24SpecialEffect2`: [特]（2）, field resonance substitution change.
- Substituted cards retain their own separate effect boundaries; do not
  flatten all copied hand/field effects into either substitution script.

#### Implementation APIs

- No complete API: implement a validated substitute-effect selection, original exclusive-card eligibility, field-state replacement, and resonance switching. Preserve this card's runtime identity rather than creating an unrelated replacement card.

### [若宫伊芙]属于我的武士道！

[资源]：

（1）此卡的[奇迹水晶]：上限3。

[手]：

依次进行以下操作：

1. 将此卡放置在[使用者]场上。
2. [投掷]12d4。
3. 获得此次[投掷]结果×60的资金。

[持续]：

（1）[使用者]抽卡后为此卡添加1个[奇迹水晶]。

（2）[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家则使用1个[奇迹水晶]并依次进行以下操作：

1. 地契主人[投掷]1d6；

2. [拥有者][投掷]3d4；

3. 投掷点数低的玩家[支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），如果[共鸣]则[支付]金额改为“投掷点差”×50。

#### Script allocation

- `PA25HandEffect`: entire [手] placement, roll, and money.
- `PA25FieldEffect1`: [持续]（1）, draw-triggered capped crystals.
- `PA25FieldEffect2`: entire [持续]（2）, crystal cost, both rolls,
  comparison, payment, and resonance formula.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd`, sequential `DiceController.ServerTryBeginRoll`, `PaymentController.ServerTryPrintMoney`, and `ServerTryPlayerPayPlayer`. Missing: crystals/resonance, multi-owner selection, and clarification of the house-count payment formula.
