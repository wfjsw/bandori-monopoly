# CiRCLE Support Team

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
CS-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Unnumbered persistent field behavior is separate from play-time resolution.

## Band Ability

### 后勤人员的努力

[自动]：

（1）自己的卡组只能加入“通用”卡。

[主动]：

（1）自己打出的“通用”卡生效时：可额外弃1张卡，使打出的那张卡的[手]效果中1个数字变为原本的2倍。将卡加入卡组的效果不适用。

#### Script allocation

- `CS01AutomaticBandEffect1`: [自动]（1）, general-card-only deck restriction.
- `CS01ActiveBandEffect1`: [主动]（1）, extra discard and eligible hand-effect parameter doubling.

#### Implementation APIs

- Call: `CardZoneController.ServerTryMoveCardToEnd` for the extra discard. Missing: general-card deck validation, private discard choice, and typed effect-parameter doubling excluding card creation; do not mutate numbers in display text.

## Characters

### 月岛麻里奈 — 礼物还有好多好多哟

[自动]：

（1）其他玩家[经过]CiRCLE时：那名玩家可[支付]自己1次500资金。若如此做，依次进行以下操作：

1. 自己[投掷]1d10。
2. 若结果至少为6，那名玩家[获得]1200资金。

#### Script allocation

- `CS02AutomaticAbility1`: [自动]（1）, automatic other-player offer, voluntary payment, source-player roll, and reward.

#### Implementation APIs

- Sequence: optional `PaymentController.ServerTryPlayerPayPlayer` (500 to Marina), `DiceController.ServerTryBeginRoll` (Marina rolls 1d10), then `PaymentController.ServerTryPrintMoney` (1200 to the passer). Missing: contribution choice and accepted-payment gating.

### 都筑诗船 — 尽力了吗

[资源]：

（1）[火罐]：初始1，上限2。

[自动]：

（1）自己每次[经过]CiRCLE时：获得1个[火罐]。

（2）游戏开始时：自己获得“Space”格子。

[主动]：

（1）自己的运营阶段：可使用1个[火罐]，使自己与1名其他玩家获得一层[尽力了吗]状态。

[尽力了吗]状态：
1.下一次用于确定移动步数的[投掷]减少[投掷]1d6的结果。若该次确定移动步数的[投掷]结果不大于0，则视为在原地进行1次[结算]。进行确定移动步数的[投掷]后减少一层该状态并获得一层[尽力了]状态和1层[SPACE的考验]状态。

[尽力了]状态：
1.下一次用于确定移动步数的[投掷]增加[投掷]1d6的结果。进行确定移动步数的[投掷]后减少一层该状态并获得1层[SPACE的考验]状态。

[SPACE的考验]状态：
1. [结算]流程中，除购买地契外，[支付]或[消耗]的资金减半；
2. 若使用者以外的拥有该状态的玩家在进行确定移动步数的[投掷]后的[结算]中加盖房屋，使用者[获得]等于该层房屋原本造价的资金。
3. [结算后]减少1层。

#### Script allocation

- `CS03AutomaticAbility1`: [自动]（1）, CiRCLE resource gain and initial amount/cap.
- `CS03AutomaticAbility2`: [自动]（2）, initial Space ownership.
- `CS03ActiveAbility1`: [主动]（1）, two-player roll modifiers, zero-step settlement, discounts, and construction reward.

#### Implementation APIs

- Calls: `TileController.ServerTryAddPropertyOwner`, `DiceController.ServerTryBeginRoll`,
  and payment multiplier/print APIs. API-01 zero-step relocation already skips passing
  and retains landing. Missing: fire pots, paired next-two-roll offsets, this skill's
  settlement/discount integration, purchase exclusion, and attributed construction rewards.

## Cards

### [月岛麻里奈]今天也要加油工作喔

[手]：

[反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。

#### Script allocation

- `CS04HandEffect`: entire [手] reaction, normal CiRCLE processing
  without the specified additional effects.

#### Implementation APIs

- No complete API: bypass only additional effects attached to tile #1 for this player's current pass/settlement while retaining its normal behavior. Global tile reset or responder unregistration would affect unrelated players and sources.

### [都筑诗船]Parking Space

[手]：

依次进行以下操作：

1. 将此卡放置于“Space”格子上。
2. 将该格子上的房屋转移至其他你拥有的格子上；每个格子因此效果最多获得1层房屋。

[持续]：

（1）此卡所在格子的[结算]改为回合结束后获得1层[停留]。

（2）位于此卡所在格子上的玩家无法使用角色及乐队技能。

#### Script allocation

- `CS05HandEffect`: entire [手] Space placement and house redistribution.
- `CS05FieldEffect1`: [持续]（1）, settlement replacement with delayed stay.
- `CS05FieldEffect2`: [持续]（2）, character/band ability prohibition on the tile.

#### Implementation APIs

- Calls: `CardZoneController.ServerTryMoveCardToEnd` and delayed `StatusController.ServerModifyStatusStacks`. Missing: house redistribution/rent refresh, custom settlement replacement, and position-scoped character/band ability suppression.
