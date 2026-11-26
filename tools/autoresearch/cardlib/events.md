# Events

## Script allocation conventions

Follow [the source/effect conventions](PoppinParty.md#script-allocation-conventions).
EV-prefixed identifiers below name separate proposed scripts, not existing
classes or runtime indexes. Entry numbers identify this proposal only.
Event scripts derive from BaseCardEffect on a neutral event source; they
are not player-hand effects. Explicit central-field behavior is separate
from initial event resolution. Use neutral zones and neutral card identity.
Where persistent text omits placement, preserve that uncertainty rather
than silently placing the event on the field.
S05 gameplay banishment uses `PlayerRelocationController.ServerTryMovePlayerOffBoard`
in addition to the authored status lifecycle; status stacks alone do not replace
movement. Expiry, return destinations, and each effect's eligibility remain explicit
consumer work. Do not apply player-owned resource assumptions to unresolved neutral
central-field crystal storage; those entry-specific gaps remain Pending.

## Cards

### 对邦

所有人立刻进行一次1d20骰子拼点，所有点数最高者从所有点数最低者获得胜出点数x50的钱（例：最高点数18，最低点数16，则需支付 (18-16)x50=100）。

#### Script allocation

- `EV01ResolutionEffect`: entire dice contest, tied-group calculation, and payments.

#### Implementation APIs

- Calls: sequential `DiceController.ServerTryBeginRoll` (1d20 each), then `PaymentController.ServerTryPlayerPayPlayer`. Resolve tied highest/lowest groups and per-pair versus shared amounts; the example specifies score difference rather than the highest score alone.

### 弦卷集团地产开发

随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得该格子的地契。

#### Script allocation

- `EV02ResolutionEffect`: entire random eligible-property selection and auction.

#### Implementation APIs

- Call: `PaymentController.ServerTryStartPropertyAuction` after selecting a valid unowned property. Missing: random eligible-tile selection and coverage for purchasable tile types outside the auction API's effective-GeneralProperty contract.

### 很噜的感觉

所有玩家可选择[传送]至快餐店（不触发场地效果），如果选择[传送]需支付500资金并喊出“噜噜噜！”（只要玩家嗓子允许，必须喊出“噜噜噜！”）。

#### Script allocation

- `EV03ResolutionEffect`: entire per-player consent, payment, and suppressed-effect teleport.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney` and `PlayerRelocationController.ServerTryTeleportPlayer` for consenting players. Missing: per-player choice and tile-effect suppression; omitting explicit settlement does not suppress relocation/pass/landing responders.

### 前往哈比内尔王国旅游

每个并获得X层[除外]，每名玩家回合开始时移除1层[除外]，结束后传送到微笑号（不触发场地效果）X=1d2（每个玩家独立投掷1d2）。

#### Script allocation

- `EV04ResolutionEffect`: per-player rolls and banishment grants with
  source-linked decay/expiry teleport behavior.
- Granted status behavior belongs to status sources; do not require the
  event to remain resolving until every player returns.
  The incomplete opening wording remains unresolved.

#### Implementation APIs

- Calls: per-player `DiceController.ServerTryBeginRoll` (1d2), `StatusController.ServerModifyStatusStacks`, and delayed teleport. Missing: source-specific turn-start decay, tile-effect suppression, and clarification of the incomplete opening text.

### 这只手我不会放开

所有玩家移动X，X=1d20-1d20+1d20，如果X是负数则向自身移动方向的反方向移动（每个玩家独立投掷三个1d20进行计算，不触发场地效果）。

#### Script allocation

- `EV05ResolutionEffect`: entire per-player three-roll calculation and signed movement.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` and `PlayerRelocationController.ServerTryMovePlayer` for each player. Preserve the three individual d20 values for a−b+c; missing: normal-direction lookup and suppression of the prohibited tile effects.

### PICO灵魂交换

所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置（不触发场地效果，例：第一位行动的玩家[传送]到第二位行动的玩家的位置并以此类推，最后行动的玩家[传送]到第一位行动的玩家的位置）。

#### Script allocation

- `EV06ResolutionEffect`: entire eligible-player/position snapshot and ordered teleports.

#### Implementation APIs

- Use S04 `MatchPlayerStateData.GetPlayerIdsSnapshot` in action order and typed
  `MatchBoardStateData.TryGetPlayerBoardPositionRecord` reads to capture eligible
  OnBoard positions before any teleport. Call
  `PlayerRelocationController.ServerTryTeleportPlayer` sequentially using that
  snapshot. Missing: event integration and tile-effect suppression; never derive
  later destinations from positions already changed by this event.

### 协助CiRCLE重建

将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃卡区，期间CiRCLE的[结算]改为选择获得2层[停留]或失去500资金（该改变[结算]的效果优先于其他任何改变[结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果。

#### Script allocation

- `EV07ResolutionEffect`: central-field placement and five-crystal initialization.
- `EV07FieldEffect`: pass-triggered decay/discard, prioritized settlement replacement,
  migrated CiRCLE behavior, and source-specific cleanup.

#### Implementation APIs

- Building blocks: `StatusController.ServerModifyStatusStacks` and `PaymentController.ServerTryDeleteMoney`. Missing: neutral central-field lifecycle, five crystals, prioritized custom settlement replacement, player choice, and reversible migration of CiRCLE's full behavior to the cafe.

### 前场队还是后场队？

所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加定位X且站在格子序号31到60的玩家的投掷结果相加定位Y；如果X=Y则所有玩家获得500资金，如果X>Y则站在格子序号1到30的玩家分摊获得1000资金，如果Y>X则站在格子序号31到60的玩家分摊获得1000资金。

#### Script allocation

- `EV08ResolutionEffect`: entire player rolls, team totals, comparison, and rewards.

#### Implementation APIs

- Calls: sequential `DiceController.ServerTryBeginRoll` and `PaymentController.ServerTryPrintMoney`. Snapshot team membership; tied teams receive 500 per player, whereas the winning team's 1000 is a shared total.

### EX任务挑战

抽出此卡的玩家投掷1d4并记录结果和26相加为X，然后所有玩家各自投掷3d20，结果至少为X的玩家可选择更换角色皮肤（技能相同）。

#### Script allocation

- `EV09ResolutionEffect`: entire threshold roll, player rolls, and eligible skin choices.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` for the drawer's 1d4 and each player's 3d20. Missing: optional in-match skin selection/publication that preserves the character and all gameplay abilities.

### 发送熊饼表情

所有技能中含有火罐的玩家可选择补充X个火罐（不能超过上限），并支付X次1000资金。随后，将一张“[衍生]冲榜”背面朝上放置于事件牌堆顶部。

#### Script allocation

- `EV10ResolutionEffect`: entire optional paid fire-pot replenishment and
  face-down insertion of [衍生]冲榜.

#### Implementation APIs

- Call: sequential `PaymentController.ServerTryDeleteMoney` for the chosen replenishments. Missing: fire-pot eligibility/caps, accepted-cost grants, and neutral-event definition insertion at deck top; player-card creation/zone APIs do not manage the neutral deck.

### 飞鸟山之战

所有玩家[传送]到飞鸟山公园。此后抽到此卡的玩家移动1d20，行动序列上在抽到此卡的玩家的上一位玩家获得2层回合开始时减少一层的[眩晕]，行动序列上在抽到此卡的玩家的下一位玩家在下个自己的回合结束前不可使用手牌，其余玩家获得1层在回合开始时移除的[除外]。

#### Script allocation

- `EV11ResolutionEffect`: entire group teleport, drawing-player movement,
  and action-order-dependent statuses/restrictions.
- Status decay and timed restriction cleanup belong to their granted
  status/modifier lifecycles, not extra event draws.

#### Implementation APIs

- Calls: sequential teleport, drawer dice/movement, and
  `StatusController.ServerModifyStatusStacks`. S04
  `MatchPlayerStateData.GetPlayerIdsSnapshot` supplies action-order snapshots.
  Missing: event-specific role assignment/integration, source-specific status
  expiry, temporary hand-play prohibition, and an explicit settlement policy.

### 幻觉来了

将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃卡区。任何玩家进行投掷前在行动顺序的上一名玩家代替进行此次投掷（所有影响投掷的效果服从于原本进行投掷的玩家所收影响）。

#### Script allocation

- `EV12ResolutionEffect`: central-field placement and drawing-player/deadline capture.
- `EV12FieldEffect`: replacement dice performer with original-player modifiers,
  and scheduled neutral discard.

#### Implementation APIs

- No complete API: implement neutral-field lifetime and a roll-performer override that preserves the original player's modifier context. Changing `DiceController.ServerTryBeginRoll`'s acting player alone would apply the wrong identity semantics.

### 卡池BUG

将此卡放置于场地中央并将一张“[衍生]修复公告”背面朝上放置于事件牌堆顶部，下次触发事件时将此卡放入事件弃卡区。此卡在场时不可在造价1500及以上的格子上加盖房屋。

#### Script allocation

- `EV13ResolutionEffect`: central-field placement and face-down 修复公告 insertion.
- `EV13FieldEffect`: construction restriction and next-event discard.

#### Implementation APIs

- No complete API: implement neutral-field placement/expiry, derived-event top insertion, and a global construction-eligibility restriction keyed to the specified development-price basis.

### A！A！O！

将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃卡区。此卡在场时全场玩家不能使用卡牌的[手]效果。

#### Script allocation

- `EV14ResolutionEffect`: central-field placement and expiry-context capture.
- `EV14FieldEffect`: hand-effect prohibition and scheduled neutral discard.

#### Implementation APIs

- No complete API: implement neutral-field lifetime and a global prohibition on card hand effects through the drawer's next-turn end. Do not suppress field/special effects or permanently unregister their sources.

### 麻里奈小姐的礼物箱

将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃卡区。所有玩家[经过]CiRCLE时可[消耗]一次500资金，抽到的玩家投掷一次1d10（不受任何其他效果影响），如果投掷结果至少为6，那名玩家[获得]1200资金。

#### Script allocation

- `EV15ResolutionEffect`: central-field placement and third-turn deadline capture.
- `EV15FieldEffect`: CiRCLE offers, costs, protected rolls/rewards,
  and scheduled neutral discard.

#### Implementation APIs

- Calls: `PaymentController.ServerTryDeleteMoney` and `ServerTryPrintMoney`. Missing: neutral-field lifetime, optional contribution gating, and an explicitly unmodifiable 1d10 roll; the ordinary dice API opens modifier/reaction windows.

### 元祖！邦多利酱

将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃卡区。所有玩家的回合免费时间变为5秒，且回合恢复时间变为0秒。

#### Script allocation

- `EV16ResolutionEffect`: central-field placement and third-turn deadline capture.
- `EV16FieldEffect`: timer overrides and source-specific expiration/discard cleanup.

#### Implementation APIs

- Call: `PlayerControlTimerController.ServerTryChangeInGameSettings`, preserving unrelated maximum-bank/enabled settings. Confirm free time maps to shieldSeconds=5 and refill to turnRefillSeconds=0; missing: neutral lifetime and source-aware restoration after overlapping modifiers.

### 超燃甩头

将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，此后抽到的玩家的下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃卡区。

#### Script allocation

- `EV17ResolutionEffect`: central-field placement and phase/deadline initialization.
- `EV17FieldEffect`: positive then negative dice-modifier phases and final discard.

#### Implementation APIs

- Call: `MovementPlanController.ServerModifyMovementDice` for the +1d6 phase during BeforeDiceRoll. Missing: two-phase neutral lifetime and subtracting a rolled d6 from the result; removing an existing d6 is not equivalent to subtracting 1d6.

### 意外的对邦

将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃卡区。所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动（如果距离一样则向正常方向移动）。

#### Script allocation

- `EV18ResolutionEffect`: central-field placement and drawing-player/deadline capture.
- `EV18FieldEffect`: other-player direction replacement and scheduled discard.

#### Implementation APIs

- No complete API: implement a temporary per-player movement-direction resolver toward the drawer, with the normal-direction tie rule, plus neutral-field expiration at the stated second turn start.

### Forbidden Moca

直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除此事件！

#### Script allocation

- `EV19ResolutionEffect`: install the timed movement-dice modifier and
  schedule permanent removal at the written deadline.
- No field placement is stated. The modifier must outlive initial resolution;
  the event's intervening zone/lifecycle remains unresolved.

#### Implementation APIs

- No complete final-roll modifier/lifetime API: halve all movement results until expiry, define rounding, and permanently remove this neutral event. `CardRemovalController.ServerRemoveCard` applies to player runtime cards, not neutral event identities.

### 上学时间

全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。

#### Script allocation

- `EV20ResolutionEffect`: entire character-school lookup/fallback, movement, and banishment.

#### Implementation APIs

- Building blocks: `PlayerRelocationController.ServerTryMovePlayer` or `ServerTryTeleportPlayer`, then `StatusController.ServerModifyStatusStacks`. Missing: authoritative character-school mapping and clarification of movement mode/path/settlement; do not assume teleport from the word move.

### 泪水的含义

所有玩家依次和行动序列里的下一位玩家进行比较：当前玩家和下一位玩家拥有同色地契则下一位玩家[支付]当前玩家700资金，否则当前玩家[支付]下一位玩家300资金。

#### Script allocation

- `EV21ResolutionEffect`: entire action-order comparison and sequential payments.

#### Implementation APIs

- Call: sequential `PaymentController.ServerTryPlayerPayPlayer` for each action-order neighbor pair. Define shared-color eligibility and snapshot/revalidation timing; preserve the stated direction and 700/300 amount for each pair.

### Kizuna Music

触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的格子并获得1层[除外]，X为所有玩家卡组里卡名为歌名的卡数量的总数。

#### Script allocation

- `EV22ResolutionEffect`: entire song-card count, roll, ordered teleports, and banishment.
  The written dice-expression ambiguity remains unresolved.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll`, sequential teleport, and `StatusController.ServerModifyStatusStacks`. Missing: authored song-card metadata/private-deck counting and clarification of X+1d20 notation, modulo-zero mapping, and settlement policy.

### [衍生]冲榜

所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一，点数最高的人获得2000资金或抽一张卡，点数第二的人获得1500资金，点数第三的人获得1000资金，点数第四的人获得500资金（如果两个人点数相同则同时获得较高的 那一档，例：获得奖励名次为1，2，2，4）。

#### Script allocation

- `EV23ResolutionEffect`: entire fire-pot removal, player rolls, tied ranking,
  and rewards/top-player choice.

#### Implementation APIs

- Calls: sequential `DiceController.ServerTryBeginRoll`, `PaymentController.ServerTryPrintMoney`, and `DrawCardController.ServerTryBeginDrawCards`. Missing: fire-pot removal/count capture and first-place reward choice; implement competition ranking such as 1,2,2,4.

### [衍生]修复公告

前2个房屋造价为2000的格子和前1个房屋造价为1500的格子的加盖金额减半，且累计在上述格子上加盖后被将此卡移除。

#### Script allocation

- `EV24ResolutionEffect`: initialize the qualifying construction discounts
  and source-specific usage/removal tracking.
- The text omits placement and has an incomplete removal condition.
  Persistent behavior requires an agreed modifier/zone lifetime; do not
  invent an independently active field effect before that is resolved.

#### Implementation APIs

- Related call: `PaymentController.ServerTryMultiplyMoneyAmount` for authorized construction discounts. Missing: qualifying construction quotas, price-basis selection, neutral-event lifetime/removal, and clarification of when all discounted uses are considered exhausted.

### [衍生]迷子的追逐

所有玩家各投5d20。如果抽到此卡的玩家的点数严格大于所有其他玩家的点数则将一张“[衍生]让我来结束一切”背面朝上放置于事件牌堆顶部，否则将一张“[衍生]一切不会结束”背面朝上放置于事件牌堆顶。

#### Script allocation

- `EV25ResolutionEffect`: entire player rolls, strict comparison, and selected
  face-down derived-event insertion.

#### Implementation APIs

- Call: sequential `DiceController.ServerTryBeginRoll` (5d20 each). Missing: conditional neutral-event top insertion and face state; compare strictly against every other player's completed result and honor the persistent suppression rule.

### [衍生]让我来结束一切

触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上，此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆顶部。

#### Script allocation

- `EV26ResolutionEffect`: three-event selection/removal and permanent field placement.
- `EV26FieldEffect`: prevent the specified later insertion by 迷子的追逐.

#### Implementation APIs

- No complete API: provide legal neutral-event selection, removal of three non-derived events, permanent neutral-field placement, and prevention of future derived copies. Do not use player-card runtime IDs or player-card zone operations.

### [衍生]一切不会结束

抽出此卡的玩家投掷1d4，所有玩家[传送]到对应数字的RiNG且获得1层在回合开始时移除的[除外]。

#### Script allocation

- `EV27ResolutionEffect`: entire drawing-player roll, group RiNG teleport,
  and status grant. Status sources own the written turn-start decay.

#### Implementation APIs

- Calls: `DiceController.ServerTryBeginRoll` (drawer, 1d4), sequential `PlayerRelocationController.ServerTryTeleportPlayer`, and `StatusController.ServerModifyStatusStacks`. Missing: authored Ring lookup, source-specific turn-start banishment removal, and explicit settlement policy.

### 火种燃尽之后会怎么样呢？

（1）将此卡的复制品放置于所有玩家所在格子上，触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格，触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡的复制品。

#### Script allocation

- `EV28Effect1`: entire （1）, initial copies, turn-start expansion,
  per-copy settlement loss, and global cleanup on banishment/bankruptcy.
- Runtime copies reuse this script implementation with distinct identities.
  Coordinator versus copy eligibility must prevent duplicate expansion
  or duplicate per-copy charges; it does not create additional numbered effects.

#### Implementation APIs

- Call: `PaymentController.ServerTryDeleteMoney` for 10 × copies on the settling tile. Missing: tile-bound neutral copies, adjacency propagation, source-linked global cleanup on banishment/bankruptcy, and clarification of duplicate placement/stacking.
