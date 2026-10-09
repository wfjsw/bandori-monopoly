// Deck editor (DeckEditorController): character + named deck list on the left,
// the current deck strip and the card pool on the right. Every change saves.
// Decks are arbitrary per character (create / duplicate / rename / delete /
// reorder); the star marks the default for deck pick. Names are profile-only
// metadata -- a match gets just the card list.

import { useEffect, useMemo, useState } from "react";
import { navigate } from "../../app/router";
import { sceneImg } from "../../core/assets";
import { sfx } from "../../core/audio";
import { cx } from "../../core/cx";
import { D, cardTitle, rules, skillText } from "../../core/data";
import { DECK_NAME_MAX, deckLabelWith, parseDeckList, sanitizeDeckName, type DeckEntry } from "../../core/deckNames";
import { useProfile } from "../../core/hooks";
import { markSeen, profileJson, updateProfile } from "../../core/store";
import type { CardData, CharacterData } from "../../core/types";
import { Btn } from "../../ui/Button";
import { CARD_KIND_LABEL, CardFace, cardKind, EmptySlot, KindChip, showCard, type CardAction, type CardKind } from "../../ui/Card";
import { BandMark, CharCard, inTab, NamePlate, tabLabels } from "../../ui/Character";
import { Chips, PanelTab } from "../../ui/Chips";
import { Form, FormRow, TextInput } from "../../ui/Form";
import { Icon } from "../../ui/Icon";
import { Live2DStand } from "../../ui/Live2DStand";
import { ask, openModal } from "../../ui/Modal";
import { SkillBody } from "../../ui/SkillBody";
import { toast } from "../../ui/Toast";
import { TopBar } from "../../ui/TopBar";
import s from "./Deck.module.css";
import { t as tr } from "../../i18n/t";

const SIZE = 10;
const DECKS_MAX = 100; // mirrors game-core deck::DECKS_MAX
const FILTERS: (CardKind | "all")[] = ["all", "exclusive", "band", "general", "derived"];
type Filter = (typeof FILTERS)[number];

const filterLabel = (f: Filter) => (f === "all" ? tr("common.all") : tr(CARD_KIND_LABEL[f]));
const matches = (c: CardData, f: Filter) => (f === "all" ? cardKind(c) !== "derived" : cardKind(c) === f);

export function Deck() {
  useEffect(() => markSeen("deck"), []);
  const p = useProfile()!; // re-render on every save
  const [character, setCharacter] = useState<CharacterData>(() => D.character(p.homeCharacter) ?? D.characters[0]);
  const [slot, setSlot] = useState(() => rules.deck_chosen_slot(profileJson(), character.name));
  const [filter, setFilter] = useState<Filter>("all");
  const [clearArmed, setClearArmed] = useState(0);
  const list: DeckEntry[] = parseDeckList(rules.deck_list(profileJson(), character.name));
  const cards: string[] = slot === 0
    ? JSON.parse(rules.deck_preset(character.name))
    : JSON.parse(rules.deck_slot_cards(profileJson(), character.name, slot));
  const chosen = rules.deck_chosen_slot(profileJson(), character.name);
  const readOnly = slot === 0;
  const label = (id: number) =>
    id === 0
      ? tr("deck.presetName")
      : deckLabelWith({ id, name: list.find((d) => d.id === id)?.name ?? "" }, (n) => tr("deck.autoName", { n }));

  const pool = useMemo(() => {
    const ids: string[] = JSON.parse(rules.deck_pool(character.name));
    const derived = D.cards.filter((c) => c.derived && (c.owner === character.name || c.band === character.band)).map((c) => c.id);
    return [...ids, ...derived.filter((id) => !ids.includes(id))].map((id) => D.card(id)!).filter(Boolean);
  }, [character]);

  const save = (n: number, next: string[]) => updateProfile(rules.deck_save(profileJson(), character.name, n, JSON.stringify(next)));
  /** Id the next create/duplicate will get (game-core allocates max+1). */
  const nextId = () => list.reduce((m, d) => Math.max(m, d.id), 0) + 1;

  const pick = (c: CharacterData) => {
    setCharacter(c);
    setSlot(rules.deck_chosen_slot(profileJson(), c.name));
    setFilter("all");
  };

  /** Append a deck and switch to it. Used by New / copy-preset / edit-from-preset. */
  function addDeck(ids: string[]): number | null {
    if (list.length >= DECKS_MAX) {
      toast(tr("deck.cap", { n: DECKS_MAX }));
      return null;
    }
    const id = nextId();
    updateProfile(rules.deck_create(profileJson(), character.name, "", JSON.stringify(ids)));
    setSlot(id);
    return id;
  }

  function toggle(id: string): void {
    const c = D.card(id)!;
    const why = rules.deck_why_not(character.name, id);
    if (why) return toast(why);
    const has = cards.includes(id);
    if (!has && cards.length >= SIZE && slot !== 0) return toast(tr("deck.fullToast", { n: SIZE }));
    const next = has ? cards.filter((x) => x !== id) : [...cards, id];
    sfx("place");
    if (slot === 0) {
      // Preset is read-only: the first edit copies it into a new deck.
      const full = !has && next.length > SIZE;
      const id2 = addDeck(full ? cards : next);
      if (id2 === null) return;
      toast(tr("deck.copiedToast", { name: label(id2) }) + (full ? tr("deck.andFix", { card: c.name }) : tr("deck.fixThen")));
      return;
    }
    const cleaned: string[] = JSON.parse(rules.deck_clean(character.name, JSON.stringify(next)));
    save(slot, cleaned);
    if (cleaned.length === SIZE && !has) toast(tr("deck.readyToast", { name: label(slot) }));
  }

  function copyPreset(): void {
    const id = addDeck(cards);
    if (id === null) return;
    toast(tr("deck.copied", { name: label(id) }));
  }

  function clearSlot(): void {
    if (!cards.length) return;
    if (Date.now() > clearArmed) {
      setClearArmed(Date.now() + 3000);
      return toast(tr("deck.clearAsk", { name: label(slot), n: cards.length }));
    }
    save(slot, []);
    setClearArmed(0);
    toast(tr("deck.cleared", { name: label(slot) }));
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

  function cardsOf(id: number): string[] {
    return id === 0
      ? JSON.parse(rules.deck_preset(character.name))
      : JSON.parse(rules.deck_slot_cards(profileJson(), character.name, id));
  }

  function setDefault(n: number): void {
    if (n !== 0 && cardsOf(n).length !== SIZE) return toast(tr("deck.setDefaultHint"));
    updateProfile(rules.deck_choose(profileJson(), character.name, n));
    toast(tr("deck.defaultToast", { name: label(n) }));
  }

  function duplicateDeck(): void {
    if (slot === 0) return copyPreset();
    if (list.length >= DECKS_MAX) return toast(tr("deck.cap", { n: DECKS_MAX }));
    const id = nextId();
    updateProfile(rules.deck_duplicate(profileJson(), character.name, slot));
    setSlot(id);
    toast(tr("deck.duplicated", { name: label(id) }));
  }

  function renameDeck(): void {
    if (slot === 0) return;
    openModal(tr("deck.renameTitle"), (close) => (
      <RenameDialog
        initial={list.find((d) => d.id === slot)?.name ?? ""}
        initialLabel={label(slot)}
        onSave={(name) => {
          updateProfile(rules.deck_rename(profileJson(), character.name, slot, name));
          toast(tr("deck.renamed", { name: name === "" ? tr("deck.autoName", { n: slot }) : name }));
        }}
        close={close}
      />
    ), { size: "small" });
  }

  async function deleteDeck(): Promise<void> {
    if (slot === 0) return;
    const name = label(slot);
    if (!(await ask(tr("deck.deleteTitle"), tr("deck.deleteAsk", { name }), tr("deck.delete")))) return;
    updateProfile(rules.deck_delete(profileJson(), character.name, slot));
    setSlot(0);
    toast(tr("deck.deleted", { name }));
  }

  function moveDeck(delta: number): void {
    if (slot === 0) return;
    updateProfile(rules.deck_move(profileJson(), character.name, slot, delta));
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
        <div className={s.slotList}>
          {[0, ...list.map((d) => d.id)].map((n) => {
            const info = n === 0
              ? tr("deck.presetInfo", { kind: character.preset?.length ? tr("deckPick.presetCustom") : tr("deckPick.presetAuto") })
              : (() => {
                  const l = cardsOf(n);
                  return !l.length ? tr("common.empty") : l.length === SIZE ? tr("deck.done") : tr("deck.short", { n: l.length, m: SIZE - l.length });
                })();
            return (
              <div key={n} className={cx(s.slot, n === slot && s.on)} onClick={() => { setSlot(n); setClearArmed(0); }}>
                <button type="button" className={cx(s.star, n === chosen && s.starOn)} title={tr("deck.setDefault")} onClick={(e) => { e.stopPropagation(); setDefault(n); }}><Icon name="star" /></button>
                <b>{label(n)}</b>
                <small>{info}</small>
              </div>
            );
          })}
        </div>
        <div className={s.slotsFoot}>
          <Btn size="small" icon="add" onClick={() => addDeck([])}>{tr("deck.new")}</Btn>
          {slot !== 0 && (
            <>
              <Btn size="small" icon="edit" onClick={renameDeck}>{tr("deck.rename")}</Btn>
              <Btn size="small" icon="redo" onClick={duplicateDeck}>{tr("deck.duplicate")}</Btn>
              <Btn size="small" icon="close" onClick={() => void deleteDeck()}>{tr("deck.delete")}</Btn>
              <Btn size="small" title={tr("deck.moveUp")} className={s.moveBtn} onClick={() => moveDeck(-1)}>↑</Btn>
              <Btn size="small" title={tr("deck.moveDown")} className={s.moveBtn} onClick={() => moveDeck(1)}>↓</Btn>
            </>
          )}
        </div>
      </div>
        </div>

        <div className={s.right}>
      <div className={s.strip}>
        <div className={s.stripHead}>
          <b>{character.display} · {label(slot)}</b>
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

/** Rename dialog: the field starts from the display name; blank restores auto. */
function RenameDialog({ initial, initialLabel, onSave, close }: {
  initial: string;
  initialLabel: string;
  onSave: (name: string) => void;
  close: () => void;
}) {
  const [name, setName] = useState(initial === "" ? initialLabel : initial);
  return (
    <Form onSubmit={() => { onSave(sanitizeDeckName(name)); close(); }}>
      <FormRow label={tr("deck.nameLabel")}>
        <TextInput value={name} maxLength={DECK_NAME_MAX} autoFocus onChange={(e) => setName(e.target.value)} />
      </FormRow>
      <small>{tr("deck.nameHint", { n: DECK_NAME_MAX })}</small>
      <div className={s.row}>
        <Btn onClick={close}>{tr("common.cancel")}</Btn>
        <Btn kind="pink" type="submit">{tr("common.confirm")}</Btn>
      </div>
    </Form>
  );
}

function Picker({ current, onPick, close }: { current: CharacterData; onPick: (c: CharacterData) => void; close: () => void }) {
  const [tab, setTab] = useState<string>(tr("common.all"));
  return (
    <div className={s.picker}>
      <Chips items={tabLabels()} on={tab as (ReturnType<typeof tabLabels>)[number]} onPick={setTab} />
      <div className={s.pickerGrid}>
        {D.characters.filter((c) => inTab(c, tab)).map((c) => {
          const built = parseDeckList(rules.deck_list(profileJson(), c.name)).length;
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