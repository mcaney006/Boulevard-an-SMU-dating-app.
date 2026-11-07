import { useEffect, useMemo, useState } from "react";
import { Compass, Heart, MessageCircle, UserRound } from "lucide-react";
import { UserButton } from "@/lib/auth/gates";
import {
  FREE_DAILY_LIKES,
  GREEKS,
  MAJORS,
  PEOPLE,
  PLUS_PRICE,
  RANK_RULES,
  YEARS,
  intentLabel,
  personById,
  type Intent,
  type Looking,
  type Person,
  type Year,
} from "@/lib/campus";
import { scorePerson } from "@/lib/rank";
import {
  loadBoulevard,
  saveProfile,
  sendMessage,
  setPlus,
  swipePerson,
  type BoulevardSnap,
  type Profile,
} from "@/lib/boulevard.fn";

type Tab = "deck" | "likes" | "matches" | "you";

const EMPTY_FILTERS = {
  intent: "any" as "any" | Intent,
  year: "any" as "any" | Year,
};

export function Boulevard() {
  const [snap, setSnap] = useState<BoulevardSnap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("deck");
  const [chatId, setChatId] = useState<string | null>(null);
  const [showPlus, setShowPlus] = useState(false);
  const [showRules, setShowRules] = useState(false);
  const [filters, setFilters] = useState(EMPTY_FILTERS);
  const [greekFilter, setGreekFilter] = useState("");
  const [majorFilter, setMajorFilter] = useState("");
  const [matchName, setMatchName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    loadBoulevard()
      .then((next) => {
        if (live) setSnap(next);
      })
      .catch((err: unknown) => {
        if (live) setError(err instanceof Error ? err.message : "Couldn't load Boulevard");
      });
    return () => {
      live = false;
    };
  }, []);

  if (error) {
    return (
      <main className="grid min-h-screen place-items-center bg-paper px-6 text-center text-ink">
        <div>
          <h1 className="font-display text-3xl">Boulevard stalled</h1>
          <p className="mt-2 text-sm text-muted">{error}</p>
        </div>
      </main>
    );
  }

  if (!snap) {
    return (
      <main className="grid min-h-screen place-items-center bg-paper text-ink">
        <p className="font-display text-2xl">Boulevard</p>
      </main>
    );
  }

  if (!snap.profile) {
    return (
      <Onboarding
        busy={busy}
        onSave={async (input) => {
          setBusy(true);
          try {
            setSnap(await saveProfile({ data: input }));
          } finally {
            setBusy(false);
          }
        }}
      />
    );
  }

  const profile = snap.profile;
  const openChat = (id: string) => {
    setChatId(id);
    setTab("matches");
  };

  return (
    <div className="min-h-screen bg-cream text-ink">
      <div className="mx-auto flex min-h-screen w-full max-w-md flex-col bg-paper shadow-none">
        <header className="flex items-center justify-between px-5 pt-5 pb-3">
          <div>
            <p className="font-display text-2xl leading-none">Boulevard</p>
            <p className="mt-1 text-xs tracking-wide text-muted">SMU · verified deck</p>
          </div>
          <button
            type="button"
            onClick={() => setShowRules(true)}
            className="h-10 rounded-full border border-line px-3 text-xs font-medium"
          >
            The score
          </button>
        </header>

        <div className="flex-1 px-4 pb-24">
          {tab === "deck" && (
            <Deck
              snap={snap}
              profile={profile}
              filters={filters}
              greekFilter={snap.plus ? greekFilter : ""}
              majorFilter={snap.plus ? majorFilter : ""}
              onFilters={setFilters}
              onGreek={setGreekFilter}
              onMajor={setMajorFilter}
              onNeedPlus={() => setShowPlus(true)}
              busy={busy}
              onSwipe={async (personId, direction) => {
                setBusy(true);
                try {
                  const result = await swipePerson({ data: { personId, direction } });
                  setSnap(result.snap);
                  if (result.matched) {
                    const person = personById(personId);
                    setMatchName(person?.name ?? "Match");
                  }
                  if (result.reason === "limit") setShowPlus(true);
                  return result.ok;
                } finally {
                  setBusy(false);
                }
              }}
            />
          )}
          {tab === "likes" && (
            <Likes
              snap={snap}
              onPlus={() => setShowPlus(true)}
              busy={busy}
              onSwipe={async (personId, direction) => {
                setBusy(true);
                try {
                  const result = await swipePerson({ data: { personId, direction } });
                  setSnap(result.snap);
                  if (result.matched) setMatchName(personById(personId)?.name ?? "Match");
                } finally {
                  setBusy(false);
                }
              }}
            />
          )}
          {tab === "matches" && !chatId && (
            <Matches snap={snap} onOpen={openChat} />
          )}
          {tab === "matches" && chatId && (
            <Chat
              snap={snap}
              personId={chatId}
              onBack={() => setChatId(null)}
              onSent={setSnap}
            />
          )}
          {tab === "you" && (
            <You
              snap={snap}
              busy={busy}
              onSave={async (input) => {
                setBusy(true);
                try {
                  setSnap(await saveProfile({ data: input }));
                } finally {
                  setBusy(false);
                }
              }}
              onPlus={() => setShowPlus(true)}
              onRules={() => setShowRules(true)}
            />
          )}
        </div>

        <nav className="fixed inset-x-0 bottom-0 z-20 mx-auto flex w-full max-w-md justify-around border-t border-line bg-paper px-2 py-2">
          <NavButton active={tab === "deck"} label="Deck" onClick={() => { setTab("deck"); setChatId(null); }}>
            <Compass className="size-5" />
          </NavButton>
          <NavButton active={tab === "likes"} label="Likes" onClick={() => { setTab("likes"); setChatId(null); }}>
            <Heart className="size-5" />
          </NavButton>
          <NavButton active={tab === "matches"} label="Chats" onClick={() => setTab("matches")}>
            <MessageCircle className="size-5" />
          </NavButton>
          <NavButton active={tab === "you"} label="You" onClick={() => { setTab("you"); setChatId(null); }}>
            <UserRound className="size-5" />
          </NavButton>
        </nav>
      </div>

      {matchName && (
        <Sheet onClose={() => setMatchName(null)}>
          <p className="text-xs tracking-wide text-crimson">Mutual</p>
          <h2 className="mt-1 font-display text-3xl">You and {matchName}</h2>
          <p className="mt-2 text-sm leading-relaxed text-muted">
            They already liked you on this deck. Their note is in the chat. Live mutuals replace this scripted set once matching runs on Cloudflare.
          </p>
          <button
            type="button"
            className="mt-5 h-12 w-full rounded-full bg-ink text-sm font-medium text-paper"
            onClick={() => {
              const person = PEOPLE.find((p) => p.name === matchName);
              setMatchName(null);
              if (person) openChat(person.id);
            }}
          >
            Open the chat
          </button>
        </Sheet>
      )}

      {showPlus && (
        <Sheet onClose={() => setShowPlus(false)}>
          <PlusPanel
            plus={snap.plus}
            onToggle={async (active) => {
              setBusy(true);
              try {
                setSnap(await setPlus({ data: { active } }));
                setShowPlus(false);
              } finally {
                setBusy(false);
              }
            }}
          />
        </Sheet>
      )}

      {showRules && (
        <Sheet onClose={() => setShowRules(false)}>
          <p className="text-xs tracking-wide text-crimson">Published ranking</p>
          <h2 className="mt-1 font-display text-3xl">Nothing hidden</h2>
          <ul className="mt-4 space-y-3 text-sm leading-relaxed">
            {RANK_RULES.map((rule) => (
              <li key={rule} className="border-b border-line pb-3">
                {rule}
              </li>
            ))}
          </ul>
          <p className="mt-4 text-xs leading-relaxed text-muted">
            The eight people on this deck are fictional stand-ins so the product can be used before the campus is live. Likes from them are labeled as scripted. Your profile, swipes, and chats are yours.
          </p>
        </Sheet>
      )}
    </div>
  );
}

function NavButton({
  active,
  label,
  onClick,
  children,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex min-h-12 min-w-16 flex-col items-center justify-center gap-0.5 text-xs ${active ? "text-crimson" : "text-muted"}`}
    >
      {children}
      {label}
    </button>
  );
}

function Sheet({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-30 flex items-end justify-center bg-ink/40 sm:items-center">
      <button type="button" aria-label="Close" className="absolute inset-0" onClick={onClose} />
      <div className="relative z-10 max-h-[85vh] w-full max-w-md overflow-y-auto rounded-t-3xl bg-card p-5 text-ink sm:rounded-3xl">
        {children}
        <button type="button" onClick={onClose} className="mt-4 h-11 w-full text-sm text-muted">
          Close
        </button>
      </div>
    </div>
  );
}

function Deck({
  snap,
  profile,
  filters,
  greekFilter,
  majorFilter,
  onFilters,
  onGreek,
  onMajor,
  onNeedPlus,
  onSwipe,
  busy,
}: {
  snap: BoulevardSnap;
  profile: Profile;
  filters: { intent: "any" | Intent; year: "any" | Year };
  greekFilter: string;
  majorFilter: string;
  onFilters: (next: { intent: "any" | Intent; year: "any" | Year }) => void;
  onGreek: (value: string) => void;
  onMajor: (value: string) => void;
  onNeedPlus: () => void;
  onSwipe: (id: string, direction: "like" | "pass") => Promise<boolean>;
  busy: boolean;
}) {
  const swiped = new Set(snap.swipes.map((s) => s.personId));
  const queue = useMemo(() => {
    return PEOPLE.filter((person) => {
      if (swiped.has(person.id)) return false;
      if (profile.lookingFor === "women" && person.gender !== "woman") return false;
      if (profile.lookingFor === "men" && person.gender !== "man") return false;
      if (filters.intent !== "any" && person.intent !== filters.intent) return false;
      if (filters.year !== "any" && person.year !== filters.year) return false;
      if (greekFilter && person.greek !== greekFilter) return false;
      if (majorFilter && person.major !== majorFilter) return false;
      return true;
    })
      .map((person) => ({
        person,
        ranked: scorePerson(
          {
            intent: profile.intent,
            major: profile.major,
            greek: profile.greek,
            year: profile.year,
          },
          person,
          snap.plus,
        ),
      }))
      .sort((a, b) => b.ranked.score - a.ranked.score || a.person.name.localeCompare(b.person.name));
  }, [filters, greekFilter, majorFilter, profile, snap.plus, snap.swipes]);

  const top = queue[0];
  const likesLeft = snap.plus ? null : Math.max(0, snap.likeLimit - snap.likesToday);

  return (
    <div>
      <div className="mb-3 flex gap-2 overflow-x-auto pb-1">
        <Select
          label="Intent"
          value={filters.intent}
          onChange={(intent) => onFilters({ ...filters, intent: intent as "any" | Intent })}
          options={[
            ["any", "Any intent"],
            ["casual", "Casual"],
            ["dating", "Dating"],
            ["relationship", "Relationship"],
          ]}
        />
        <Select
          label="Year"
          value={filters.year}
          onChange={(year) => onFilters({ ...filters, year: year as "any" | Year })}
          options={[["any", "Any year"], ...YEARS.map((y) => [y, y] as [string, string])]}
        />
      </div>
      <div className="mb-3 flex gap-2">
        <LockedSelect
          plus={snap.plus}
          label="House"
          value={greekFilter}
          onChange={onGreek}
          onLocked={onNeedPlus}
          options={[["", "Any house"], ...GREEKS.filter((g) => g !== "Independent").map((g) => [g, g] as [string, string])]}
        />
        <LockedSelect
          plus={snap.plus}
          label="Major"
          value={majorFilter}
          onChange={onMajor}
          onLocked={onNeedPlus}
          options={[["", "Any major"], ...MAJORS.map((m) => [m, m] as [string, string])]}
        />
      </div>

      <p className="mb-3 text-xs text-muted">
        {snap.plus
          ? "Boulevard+ · unlimited likes · like-signal visible"
          : `${likesLeft} free likes left today · ${FREE_DAILY_LIKES} cap`}
      </p>

      {!top ? (
        <div className="rounded-card border border-line bg-card p-6">
          <h2 className="font-display text-3xl">Deck’s clear</h2>
          <p className="mt-2 text-sm leading-relaxed text-muted">
            Nobody left who matches these filters. Loosen them, or wait for a live campus instead of this eight-person stand-in.
          </p>
        </div>
      ) : (
        <Card
          person={top.person}
          score={top.ranked.score}
          reasons={top.ranked.reasons}
          busy={busy}
          onPass={() => void onSwipe(top.person.id, "pass")}
          onLike={() => void onSwipe(top.person.id, "like")}
        />
      )}
    </div>
  );
}

function Card({
  person,
  score,
  reasons,
  busy,
  onPass,
  onLike,
}: {
  person: Person;
  score: number;
  reasons: { label: string; points: number }[];
  busy: boolean;
  onPass: () => void;
  onLike: () => void;
}) {
  return (
    <article className="overflow-hidden rounded-card border border-line bg-card">
      <img
        src={person.photo}
        alt=""
        className="aspect-[3/4] w-full object-cover"
      />
      <div className="space-y-3 p-4">
        <div className="flex items-end justify-between gap-3">
          <div>
            <h2 className="font-display text-3xl leading-none">{person.name}</h2>
            <p className="mt-1 text-sm text-muted">
              {person.year} · {person.major} · {person.school}
            </p>
          </div>
          <p className="font-display text-2xl text-crimson">{score}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Pill>{intentLabel(person.intent)}</Pill>
          <Pill>{person.greek || "Independent"}</Pill>
        </div>
        <p className="text-sm leading-relaxed">{person.bio}</p>
        <div className="rounded-2xl bg-cream px-3 py-2">
          <p className="text-xs tracking-wide text-brass">Why this order</p>
          {reasons.length === 0 ? (
            <p className="mt-1 text-sm text-muted">No overlap with your profile. They sort after everyone who matches a rule.</p>
          ) : (
            <ul className="mt-1 space-y-1 text-sm">
              {reasons.map((r) => (
                <li key={r.label} className="flex justify-between">
                  <span>{r.label}</span>
                  <span className="text-crimson">+{r.points}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="flex gap-2 pt-1">
          <button
            type="button"
            disabled={busy}
            onClick={onPass}
            className="h-12 flex-1 rounded-full border border-line text-sm font-medium disabled:opacity-50"
          >
            Pass
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={onLike}
            className="h-12 flex-1 rounded-full bg-crimson text-sm font-medium text-paper disabled:opacity-50"
          >
            Like
          </button>
        </div>
      </div>
    </article>
  );
}

function Likes({
  snap,
  onPlus,
  onSwipe,
  busy,
}: {
  snap: BoulevardSnap;
  onPlus: () => void;
  busy: boolean;
  onSwipe: (id: string, direction: "like" | "pass") => Promise<void>;
}) {
  const decided = new Set(snap.swipes.map((s) => s.personId));
  const waiting = PEOPLE.filter((p) => p.likesYou && !decided.has(p.id));

  if (!snap.plus) {
    return (
      <section>
        <h2 className="font-display text-3xl">Who liked you</h2>
        <p className="mt-2 text-sm leading-relaxed text-muted">
          {waiting.length === 0
            ? "Nobody on this deck is waiting. That count is real for the stand-in set."
            : `${waiting.length} on this deck already liked you. Names stay hidden until Boulevard+.`}
        </p>
        <div className="mt-4 space-y-3">
          {waiting.map((person) => (
            <div key={person.id} className="flex items-center gap-3 rounded-2xl border border-line bg-card p-3">
              <div className="size-14 rounded-xl bg-cream" />
              <div>
                <p className="text-sm font-medium">Hidden until Plus</p>
                <p className="text-xs text-muted">{person.school} · {intentLabel(person.intent)}</p>
              </div>
            </div>
          ))}
        </div>
        <button type="button" onClick={onPlus} className="mt-4 h-12 w-full rounded-full bg-ink text-sm font-medium text-paper">
          See them · {PLUS_PRICE}
        </button>
      </section>
    );
  }

  return (
    <section className="space-y-3">
      <h2 className="font-display text-3xl">Who liked you</h2>
      {waiting.length === 0 && <p className="text-sm text-muted">No unanswered likes on the deck.</p>}
      {waiting.map((person) => (
        <article key={person.id} className="overflow-hidden rounded-2xl border border-line bg-card">
          <img src={person.photo} alt="" className="h-44 w-full object-cover object-top" />
          <div className="p-3">
            <h3 className="font-display text-2xl">{person.name}</h3>
            <p className="text-sm text-muted">{person.year} · {person.major}</p>
            {person.note && <p className="mt-2 text-sm leading-relaxed">“{person.note}”</p>}
            <div className="mt-3 flex gap-2">
              <button type="button" disabled={busy} onClick={() => void onSwipe(person.id, "pass")} className="h-11 flex-1 rounded-full border border-line text-sm">
                Pass
              </button>
              <button type="button" disabled={busy} onClick={() => void onSwipe(person.id, "like")} className="h-11 flex-1 rounded-full bg-crimson text-sm text-paper">
                Like back
              </button>
            </div>
          </div>
        </article>
      ))}
    </section>
  );
}

function Matches({ snap, onOpen }: { snap: BoulevardSnap; onOpen: (id: string) => void }) {
  const matches = snap.swipes
    .filter((s) => s.direction === "like")
    .map((s) => personById(s.personId))
    .filter((p): p is Person => Boolean(p && p.likesYou));

  return (
    <section>
      <h2 className="font-display text-3xl">Chats</h2>
      <p className="mt-2 text-sm text-muted">A chat opens only after a mutual like. No message requests from passes.</p>
      <div className="mt-4 space-y-2">
        {matches.length === 0 && (
          <p className="rounded-2xl border border-line bg-card p-4 text-sm text-muted">
            No mutuals yet. Like someone who already liked you.
          </p>
        )}
        {matches.map((person) => {
          const last = [...snap.messages].reverse().find((m) => m.personId === person.id);
          return (
            <button
              key={person.id}
              type="button"
              onClick={() => onOpen(person.id)}
              className="flex w-full items-center gap-3 rounded-2xl border border-line bg-card p-3 text-left"
            >
              <img src={person.photo} alt="" className="size-12 rounded-xl object-cover" />
              <span className="min-w-0">
                <span className="block font-medium">{person.name}</span>
                <span className="block truncate text-sm text-muted">{last?.body ?? "New mutual"}</span>
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function Chat({
  snap,
  personId,
  onBack,
  onSent,
}: {
  snap: BoulevardSnap;
  personId: string;
  onBack: () => void;
  onSent: (snap: BoulevardSnap) => void;
}) {
  const person = personById(personId);
  const [body, setBody] = useState("");
  const [sending, setSending] = useState(false);
  const thread = snap.messages.filter((m) => m.personId === personId);

  if (!person) return null;

  return (
    <section className="flex min-h-[70vh] flex-col">
      <button type="button" onClick={onBack} className="mb-3 self-start text-sm text-muted">
        Back to chats
      </button>
      <div className="flex items-center gap-3">
        <img src={person.photo} alt="" className="size-12 rounded-xl object-cover" />
        <div>
          <h2 className="font-display text-2xl leading-none">{person.name}</h2>
          <p className="text-xs text-muted">{person.major}</p>
        </div>
      </div>
      <div className="mt-4 flex flex-1 flex-col gap-2">
        {thread.length === 0 && (
          <p className="text-sm text-muted">No note yet. Say something specific.</p>
        )}
        {thread.map((m) => (
          <p
            key={m.id}
            className={`max-w-[85%] rounded-2xl px-3 py-2 text-sm leading-relaxed ${m.fromMe ? "self-end bg-ink text-paper" : "self-start bg-cream text-ink"}`}
          >
            {m.body}
          </p>
        ))}
      </div>
      <form
        className="mt-4 flex gap-2"
        onSubmit={async (event) => {
          event.preventDefault();
          const text = body.trim();
          if (!text || sending) return;
          setSending(true);
          try {
            const result = await sendMessage({ data: { personId, body: text } });
            if (result.ok) {
              setBody("");
              onSent(result.snap);
            }
          } finally {
            setSending(false);
          }
        }}
      >
        <input
          value={body}
          onChange={(event) => setBody(event.target.value)}
          maxLength={500}
          placeholder="Message"
          className="h-12 flex-1 rounded-full border border-line bg-card px-4 text-sm outline-none"
        />
        <button type="submit" disabled={sending || !body.trim()} className="h-12 rounded-full bg-crimson px-4 text-sm text-paper disabled:opacity-50">
          Send
        </button>
      </form>
    </section>
  );
}

function You({
  snap,
  busy,
  onSave,
  onPlus,
  onRules,
}: {
  snap: BoulevardSnap;
  busy: boolean;
  onSave: (input: {
    displayName: string;
    year: Year;
    major: (typeof MAJORS)[number];
    greek: (typeof GREEKS)[number];
    intent: Intent;
    lookingFor: Looking;
    bio: string;
  }) => Promise<void>;
  onPlus: () => void;
  onRules: () => void;
}) {
  const profile = snap.profile;
  if (!profile) return null;
  return (
    <section>
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 className="font-display text-3xl">{profile.displayName}</h2>
          <p className="mt-1 text-sm text-muted">
            {profile.year} · {profile.major} · {profile.greek || "Independent"}
          </p>
        </div>
        <UserButton />
      </div>
      <p className="mt-3 text-sm leading-relaxed">{profile.bio}</p>
      <p className="mt-3 text-sm">
        Showing {profile.lookingFor === "everyone" ? "everyone" : profile.lookingFor} · {intentLabel(profile.intent)}
      </p>
      <div className="mt-4 flex gap-2">
        <button type="button" onClick={onPlus} className="h-11 flex-1 rounded-full bg-ink text-sm text-paper">
          {snap.plus ? "Boulevard+ is on" : `Boulevard+ · ${PLUS_PRICE}`}
        </button>
        <button type="button" onClick={onRules} className="h-11 flex-1 rounded-full border border-line text-sm">
          Ranking rules
        </button>
      </div>
      <div className="mt-6">
        <ProfileForm
          initial={{
            displayName: profile.displayName,
            year: profile.year,
            major: (MAJORS as readonly string[]).includes(profile.major)
              ? (profile.major as (typeof MAJORS)[number])
              : "Finance",
            greek: profile.greek
              ? (GREEKS as readonly string[]).includes(profile.greek)
                ? (profile.greek as (typeof GREEKS)[number])
                : "Independent"
              : "Independent",
            intent: profile.intent,
            lookingFor: profile.lookingFor,
            bio: profile.bio,
          }}
          busy={busy}
          submitLabel="Save profile"
          onSave={onSave}
        />
      </div>
    </section>
  );
}

function Onboarding({
  busy,
  onSave,
}: {
  busy: boolean;
  onSave: (input: {
    displayName: string;
    year: Year;
    major: (typeof MAJORS)[number];
    greek: (typeof GREEKS)[number];
    intent: Intent;
    lookingFor: Looking;
    bio: string;
  }) => Promise<void>;
}) {
  return (
    <main className="min-h-screen bg-paper px-5 py-8 text-ink">
      <div className="mx-auto max-w-md">
        <p className="text-xs tracking-wide text-crimson">Boulevard · SMU</p>
        <h1 className="mt-2 font-display text-4xl leading-none">Say what you actually want.</h1>
        <p className="mt-3 text-sm leading-relaxed text-muted">
          This profile is what the published score uses. Intent, major, house, and year are the only inputs. No secret rating.
        </p>
        <div className="mt-6">
          <ProfileForm
            initial={{
              displayName: "",
              year: "Junior",
              major: "Finance",
              greek: "Independent",
              intent: "dating",
              lookingFor: "women",
              bio: "",
            }}
            busy={busy}
            submitLabel="Enter the deck"
            onSave={onSave}
          />
        </div>
      </div>
    </main>
  );
}

function ProfileForm({
  initial,
  busy,
  submitLabel,
  onSave,
}: {
  initial: {
    displayName: string;
    year: Year;
    major: (typeof MAJORS)[number];
    greek: (typeof GREEKS)[number];
    intent: Intent;
    lookingFor: Looking;
    bio: string;
  };
  busy: boolean;
  submitLabel: string;
  onSave: (input: typeof initial) => Promise<void>;
}) {
  const [form, setForm] = useState(initial);
  return (
    <form
      className="space-y-3"
      onSubmit={(event) => {
        event.preventDefault();
        void onSave(form);
      }}
    >
      <Field label="Name">
        <input
          required
          maxLength={40}
          value={form.displayName}
          onChange={(event) => setForm({ ...form, displayName: event.target.value })}
          className="h-12 w-full rounded-2xl border border-line bg-card px-3 text-sm outline-none"
        />
      </Field>
      <Field label="Year">
        <select
          value={form.year}
          onChange={(event) => setForm({ ...form, year: event.target.value as Year })}
          className="h-12 w-full rounded-2xl border border-line bg-card px-3 text-sm"
        >
          {YEARS.map((year) => (
            <option key={year}>{year}</option>
          ))}
        </select>
      </Field>
      <Field label="Major">
        <select
          value={form.major}
          onChange={(event) => setForm({ ...form, major: event.target.value as (typeof MAJORS)[number] })}
          className="h-12 w-full rounded-2xl border border-line bg-card px-3 text-sm"
        >
          {MAJORS.map((major) => (
            <option key={major}>{major}</option>
          ))}
        </select>
      </Field>
      <Field label="House">
        <select
          value={form.greek}
          onChange={(event) => setForm({ ...form, greek: event.target.value as (typeof GREEKS)[number] })}
          className="h-12 w-full rounded-2xl border border-line bg-card px-3 text-sm"
        >
          {GREEKS.map((greek) => (
            <option key={greek}>{greek}</option>
          ))}
        </select>
      </Field>
      <Field label="Intent">
        <select
          value={form.intent}
          onChange={(event) => setForm({ ...form, intent: event.target.value as Intent })}
          className="h-12 w-full rounded-2xl border border-line bg-card px-3 text-sm"
        >
          <option value="casual">Something casual</option>
          <option value="dating">Actually dating</option>
          <option value="relationship">A relationship</option>
        </select>
      </Field>
      <Field label="Show me">
        <select
          value={form.lookingFor}
          onChange={(event) => setForm({ ...form, lookingFor: event.target.value as Looking })}
          className="h-12 w-full rounded-2xl border border-line bg-card px-3 text-sm"
        >
          <option value="women">Women</option>
          <option value="men">Men</option>
          <option value="everyone">Everyone</option>
        </select>
      </Field>
      <Field label="Bio">
        <textarea
          required
          maxLength={280}
          value={form.bio}
          onChange={(event) => setForm({ ...form, bio: event.target.value })}
          rows={4}
          className="w-full rounded-2xl border border-line bg-card px-3 py-3 text-sm outline-none"
        />
      </Field>
      <button
        type="submit"
        disabled={busy || !form.displayName.trim() || !form.bio.trim()}
        className="h-12 w-full rounded-full bg-crimson text-sm font-medium text-paper disabled:opacity-50"
      >
        {submitLabel}
      </button>
    </form>
  );
}

function PlusPanel({
  plus,
  onToggle,
}: {
  plus: boolean;
  onToggle: (active: boolean) => Promise<void>;
}) {
  return (
    <div>
      <p className="text-xs tracking-wide text-crimson">Boulevard+</p>
      <h2 className="mt-1 font-display text-3xl">{PLUS_PRICE}</h2>
      <ul className="mt-4 space-y-2 text-sm leading-relaxed">
        <li>Unlimited likes. Free stays at {FREE_DAILY_LIKES} a day.</li>
        <li>See who already liked you, with their note.</li>
        <li>House and major filters.</li>
        <li>The “already liked you” points show up in the score before you swipe.</li>
      </ul>
      <p className="mt-4 text-xs leading-relaxed text-muted">
        This preview does not charge a card. Turning Plus on here only unlocks those four things on your account. Real billing waits for the deploy.
      </p>
      <button
        type="button"
        onClick={() => void onToggle(!plus)}
        className="mt-5 h-12 w-full rounded-full bg-ink text-sm font-medium text-paper"
      >
        {plus ? "Turn Plus off" : "Unlock Plus in this preview"}
      </button>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block text-xs tracking-wide text-muted">{label}</span>
      {children}
    </label>
  );
}

function Pill({ children }: { children: React.ReactNode }) {
  return <span className="rounded-full bg-crimson-soft px-3 py-1 text-xs text-crimson">{children}</span>;
}

function Select({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: [string, string][];
}) {
  return (
    <label className="text-xs text-muted">
      {label}
      <select
        value={value}
        aria-label={label}
        onChange={(event) => onChange(event.target.value)}
        className="mt-1 block h-10 rounded-full border border-line bg-card px-3 text-sm text-ink"
      >
        {options.map(([id, name]) => (
          <option key={id} value={id}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

function LockedSelect({
  plus,
  label,
  value,
  onChange,
  onLocked,
  options,
}: {
  plus: boolean;
  label: string;
  value: string;
  onChange: (value: string) => void;
  onLocked: () => void;
  options: [string, string][];
}) {
  if (!plus) {
    return (
      <button type="button" onClick={onLocked} className="h-10 rounded-full border border-dashed border-line px-3 text-xs text-muted">
        {label} · Plus
      </button>
    );
  }
  return <Select label={label} value={value} onChange={onChange} options={options} />;
}
