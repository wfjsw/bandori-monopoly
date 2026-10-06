// Deck editor (DeckEditorController): character card + slot list on the left,
// the current deck strip and the card pool on the right. Every change saves.

import { useEffect, useMemo, useState } from "react";
import { navigate } from "../../app/router";
import { sceneImg } from "../../core/assets";
import { sfx } from "../../core/audio";
import { cx } from "../../core/cx";
import { D, cardTitle, rules, skillText } from "../../core/data";
import { useProfile } from "../../core/hooks";
import { markSeen, profileJson, updateProfile } from "../../core/store";
import type { CardData, CharacterData } from "../../core/types";
import { Btn } from "../../ui/Button";
import { CARD_KIND_LABEL, CardFace, cardKind, EmptySlot, KindChip, showCard, type CardAction, type CardKind } from "../../ui/Card";
import { BandMark, CharCard, inTab, NamePlate, tabLabels } from "../../ui/Character";
import { Chips, PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { Live2DStand } from "../../ui/Live2DStand";
import { openModal } from "../../ui/Modal";
import { SkillBody } from "../../ui/SkillBody";
import { toast } from "../../ui/Toast";
import { TopBar } from "../../ui/TopBar";
import s from "./Deck.module.css";
import { t as tr } from "../../i18n/t";

const SIZE = 10;
const slotName = (n: number) => tr("deck.slotNames").split("|")[n] ?? `#${n}`;
const FILTERS: (CardKind | "all")[] = ["all", "exclusive", "band", "general", "derived"];
type Filter = (typeof FILTERS)[number];

const filterLabel = (f: Filter) => (f === "all" ? tr("common.all") : tr(CARD_KIND_LABEL[f]));
const matches = (c: CardData, f: Filter) => (f === "all" ? cardKind(c) !== "derived" : cardKind(c) === f);
const slotCards = (ch: string, n: number): string[] =>
  n === 0 ? JSON.parse(rules.deck_preset(ch)) : JSON.parse(rules.deck_slot_cards(profileJson(), ch, n));

export function Deck() {
  useEffect(() => markSeen("deck"), []);
  const p = useProfile()!; // re-render on every save
  const [character, setCharacter] = useState<CharacterData>(() => D.character(p.homeCharacter) ?? D.characters[0]);
  const [slot, setSlot] = useState(() => rules.deck_chosen_slot(profileJson(), character.name));
  const [filter, setFilter] = useState<Filter>("all");
  const [clearArmed, setClearArmed] = useState(0);
  const cards = slotCards(character.name, slot);
  const chosen = rules.deck_chosen_slot(profileJson(), character.name);
  const readOnly = slot === 0;

  const pool = useMemo(() => {
    const ids: string[] = JSON.parse(rules.deck_pool(character.name));
    const derived = D.cards.filter((c) => c.derived && (c.owner === character.name || c.band === character.band)).map((c) => c.id);
    return [...ids, ...derived.filter((id) => !ids.includes(id))].map((id) => D.card(id)!).filter(Boolean);
  }, [character]);

  const save = (n: number, next: string[]) => updateProfile(rules.deck_save(profileJson(), character.name, n, JSON.stringify(next)));
  const firstEmpty = () => [1, 2, 3].find((n) => !slotCards(character.name, n).length) ?? 0;

  const pick = (c: CharacterData) => {
    setCharacter(c);
    setSlot(rules.deck_chosen_slot(profileJson(), c.name));
    setFilter("all");
  };

  function toggle(id: string): void {
    const c = D.card(id)!;
    const why = rules.deck_why_not(character.name, id);
    if (why) return toast(why);
    const has = cards.includes(id);
    if (!has && cards.length >= SIZE && slot !== 0) return toast(tr("deck.fullToast", { n: SIZE }));
    const next = has ? cards.filter((x) => x !== id) : [...cards, id];
    sfx("place");
    if (slot === 0) {
      const n = firstEmpty();
      if (!n) return toast(tr("deck.allUsedEdit"));
      const full = !has && next.length > SIZE;
      save(n, full ? cards : next);
      setSlot(n);
      toast(tr("deck.copiedToast", { name: slotName(n) }) + (full ? tr("deck.andFix", { card: c.name }) : tr("deck.fixThen")));
      return;
    }
    const cleaned: string[] = JSON.parse(rules.deck_clean(character.name, JSON.stringify(next)));
    save(slot, cleaned);
    if (cleaned.length === SIZE && !has) toast(tr("deck.readyToast", { name: slotName(slot) }));
  }

  function copyPreset(): void {
    const n = firstEmpty();
    if (!n) return toast(tr("deck.allUsed"));
    save(n, cards);
    setSlot(n);
    toast(tr("deck.copied", { name: slotName(n) }));
  }

  function clearSlot(): void {
    if (!cards.length) return;
    if (Date.now() > clearArmed) {
      setClearArmed(Date.now() + 3000);
      return toast(tr("deck.clearAsk", { name: slotName(slot), n: cards.length }));
    }
    save(slot, []);
    setClearArmed(0);
    toast(tr("deck.cleared", { name: slotName(slot) }));
  }

  function fill(): void {
    if (cards.length >= SIZE) return toast(tr("deck.filledNone"));
    const next = [...cards];
    for (const c of pool) {
      if (next.length >= SIZE) break;
      if (!c.derived && !next.includes(c.id) && !rules.deck_why_not(character.name, c.id)) next.push(c.id);
    }
    const cleaned: string[] = JSON.parse(rules.deck_clean(character.name, JSON.stringify(next)));
    save(slot, cleaned);
    toast(tr("deck.filled", { n: cleaned.length - cards.length }));
  }

  function setDefault(n: number): void {
    if (n !== 0 && slotCards(character.name, n).length !== SIZE) return toast(tr("deck.setDefaultHint"));
    updateProfile(rules.deck_choose(profileJson(), character.name, n));
    toast(tr("deck.defaultToast", { name: slotName(n) }));
  }

  function detail(id: string): void {
    const c = D.card(id)!;
    const has = cards.includes(id);
    const acts: CardAction[] = [];
    if (c.derived) acts.push({ label: tr("deck.cannotAdd"), enabled: false });
    else if (has) acts.push({ label: slot === 0 ? tr("deck.removePreset") : tr("deck.remove"), enabled: true, run: () => toggle(id) });
    else if (slot === 0) acts.push({ label: tr("deck.swapPreset"), enabled: true, run: () => toggle(id) });
    else acts.push({ label: cards.length < SIZE ? tr("deck.add") : tr("deck.full"), enabled: cards.length < SIZE, run: () => toggle(id) });
    showCard(id, acts);
  }

  const counts = FILTERS.map((f) => pool.filter((c) => matches(c, f)).length);
  return (
    <>
      <TopBar section={tr("deck.section")} title={tr("deck.title")} onBack={() => navigate({ name: "menu" })} />
      <div className={s.body}>
        <div className={s.left}>
      <div className={s.char}>
        <div className={s.glow} style={{ background: character.color }} />
        <Live2DStand key={D.artId(character)} id={D.artId(character)} className={s.stand} zoom={1.25} focusTop={0.13} headroom={0.103} />
        <div className={s.band}><BandMark band={character.band} /></div>
        <div className={s.plate}><NamePlate c={character} width={350} /></div>
        <Btn icon="swap_horiz" className={s.side} onClick={() => showPicker(character, pick)}>{tr("deck.changeChar")}</Btn>
        <Btn icon="groups" className={cx(s.side, s.second)} onClick={() => showBandCards(character)}>{tr("deck.bandCards")}</Btn>
      </div>

      <div className={s.slots}>
        <div className={s.slotsHead}>
          <PanelTab>{tr("deck.slotTab")}</PanelTab>
          <img src={sceneImg("icon_star")} alt="" />
          <small>{tr("deck.defaultHint")}</small>
        </div>
        {[0, 1, 2, 3].map((n) => {
          const list = slotCards(character.name, n);
          const info = n === 0 ? tr("deck.presetInfo", { kind: character.preset?.length ? tr("deckPick.presetCustom") : tr("deckPick.presetAuto") }) : !list.length ? tr("common.empty") : list.length === SIZE ? tr("deck.done") : tr("deck.short", { n: list.length, m: SIZE - list.length });
          return (
            <div key={n} className={cx(s.slot, n === slot && s.on)} onClick={() => { setSlot(n); setClearArmed(0); }}>
              <button type="button" className={cx(s.star, n === chosen && s.starOn)} title={tr("deck.setDefault")} onClick={(e) => { e.stopPropagation(); setDefault(n); }}><Icon name="star" /></button>
              <b>{slotName(n)}</b>
              <small>{info}</small>
            </div>
          );
        })}
      </div>
        </div>

        <div className={s.right}>
      <div className={s.strip}>
        <div className={s.stripHead}>
          <b>{character.display} · {slotName(slot)}</b>
          <div className={s.count}><span className={cards.length === SIZE ? s.full : ""}>{cards.length}</span> / {SIZE}</div>
          {readOnly
            ? <Btn kind="pink" size="small" icon="edit" onClick={copyPreset}>{tr("deck.copyPreset")}</Btn>
            : <div className={s.row}><Btn size="small" icon="auto_awesome" onClick={fill}>{tr("deck.fill")}</Btn><Btn size="small" icon="close" onClick={clearSlot}>{tr("deck.clear")}</Btn></div>}
        </div>
        <div className={s.stripCards}>
          {Array.from({ length: SIZE }, (_, k) => (cards[k]
            ? <CardFace key={k} id={cards[k]} size="strip" onClick={() => detail(cards[k])} />
            : <EmptySlot key={k} star={sceneImg("star5")} />))}
        </div>
        <div className={s.hint}>
          {readOnly ? tr("deck.hintPreset")
            : cards.length === SIZE ? tr("deck.hintDone") : tr("deck.hintBuild")}
        </div>
      </div>

      <div className={s.pool}>
        <div className={s.poolHead}>
          <Chips items={FILTERS.filter((_, k) => k < 4 || counts[k] > 0).map(filterLabel)} on={filterLabel(filter)} count={(l) => counts[FILTERS.findIndex((f) => filterLabel(f) === l)]} onPick={(l) => setFilter(FILTERS.find((f) => filterLabel(f) === l) ?? "all")} />
          <small>{tr("deck.poolHint")}</small>
        </div>
        <div className={s.grid}>
          {pool.filter((c) => matches(c, filter)).map((c) => {
            const on = cards.includes(c.id);
            return (
              <CardFace key={c.id} id={c.id} size="pool" on={on} onClick={() => detail(c.id)} title={cardTitle(c.id)}>
                <KindChip kind={cardKind(c)} className={s.kind} />
                {!c.derived && (
                  <button type="button" className={cx(s.toggle, on && s.toggleOn)} onClick={(e) => { e.stopPropagation(); toggle(c.id); }}>
                    <Icon name={on ? "check" : "add"} />
                  </button>
                )}
              </CardFace>
            );
          })}
        </div>
      </div>
        </div>
      </div>
    </>
  );
}

function Picker({ current, onPick, close }: { current: CharacterData; onPick: (c: CharacterData) => void; close: () => void }) {
  const [tab, setTab] = useState<string>(tr("common.all"));
  return (
    <div className={s.picker}>
      <Chips items={tabLabels()} on={tab as (ReturnType<typeof tabLabels>)[number]} onPick={setTab} />
      <div className={s.pickerGrid}>
        {D.characters.filter((c) => inTab(c, tab)).map((c) => {
          const built = [1, 2, 3].filter((n) => slotCards(c.name, n).length).length;
          return <CharCard key={c.name} c={c} chosen={c.name === current.name} stamp={built ? tr("deck.builtCount", { n: built }) : undefined} stampKind="teal" onClick={() => { close(); onPick(c); }} />;
        })}
      </div>
    </div>
  );
}

function showPicker(current: CharacterData, onPick: (c: CharacterData) => void): void {
  openModal(tr("deck.changeChar"), (close) => <Picker current={current} onPick={onPick} close={close} />, { size: "xl" });
}

function showBandCards(c: CharacterData): void {
  const band = D.band(c.band);
  openModal(tr("deck.bandCards"), (
    <div className={s.bandCards}>
      <div className={s.row}><BandMark band={c.band} /><b>{band?.skill ?? ""}</b></div>
      <SkillBody text={skillText(band) || tr("select.bandSkillTbd")} />
      <div className={s.bandGrid}>
        {D.cards.filter((x) => x.band === c.band && !x.owner && !x.derived).map((x) => <CardFace key={x.id} id={x.id} onClick={() => showCard(x.id)} />)}
      </div>
    </div>
  ), { size: "wide" });
}
