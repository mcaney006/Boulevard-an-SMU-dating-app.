import { createServerFn } from "@tanstack/react-start";
import { z } from "zod";
import { authMiddleware } from "@/lib/auth/middleware";
import {
  FREE_DAILY_LIKES,
  GREEKS,
  INTENTS,
  LOOKING,
  MAJORS,
  PEOPLE,
  PERSON_IDS,
  YEARS,
  personById,
  type Intent,
  type Looking,
  type Year,
} from "@/lib/campus";
import { getSql } from "@/lib/db";

const profileInput = z.object({
  displayName: z.string().trim().min(1).max(40),
  year: z.enum(YEARS),
  major: z.enum(MAJORS),
  greek: z.enum(GREEKS),
  intent: z.enum(INTENTS),
  lookingFor: z.enum(LOOKING),
  bio: z.string().trim().min(1).max(280),
});

export type Profile = {
  displayName: string;
  year: Year;
  major: string;
  greek: string;
  intent: Intent;
  lookingFor: Looking;
  bio: string;
};

export type Swipe = { personId: string; direction: "like" | "pass" };
export type ChatMessage = {
  id: number;
  personId: string;
  body: string;
  fromMe: boolean;
  createdAt: string;
};

export type BoulevardSnap = {
  profile: Profile | null;
  plus: boolean;
  swipes: Swipe[];
  messages: ChatMessage[];
  likesToday: number;
  likeLimit: number;
};

type ProfileRow = {
  display_name: string;
  year_label: string;
  major: string;
  greek: string;
  intent: string;
  looking_for: string;
  bio: string;
};

function toProfile(row: ProfileRow): Profile {
  return {
    displayName: row.display_name,
    year: row.year_label as Year,
    major: row.major,
    greek: row.greek,
    intent: row.intent as Intent,
    lookingFor: row.looking_for as Looking,
    bio: row.bio,
  };
}

async function readSnap(userId: string): Promise<BoulevardSnap> {
  const sql = await getSql();
  const profiles = await sql<ProfileRow>`
    select display_name, year_label, major, greek, intent, looking_for, bio
    from boulevard_profiles
    where user_id = ${userId}
  `;
  const plusRows = await sql<{ active: boolean }>`
    select active from boulevard_plus where user_id = ${userId}
  `;
  const swipes = await sql<{ person_id: string; direction: string }>`
    select person_id, direction from boulevard_swipes where user_id = ${userId}
  `;
  const messages = await sql<{
    id: number;
    person_id: string;
    body: string;
    from_me: boolean;
    created_at: string;
  }>`
    select id, person_id, body, from_me, created_at::text as created_at
    from boulevard_messages
    where user_id = ${userId}
    order by id asc
  `;
  const counts = await sql<{ n: number }>`
    select count(*)::int as n
    from boulevard_swipes
    where user_id = ${userId}
      and direction = 'like'
      and created_at >= date_trunc('day', now())
  `;

  return {
    profile: profiles[0] ? toProfile(profiles[0]) : null,
    plus: Boolean(plusRows[0]?.active),
    swipes: swipes
      .filter((s) => s.direction === "like" || s.direction === "pass")
      .map((s) => ({
        personId: s.person_id,
        direction: s.direction as "like" | "pass",
      })),
    messages: messages.map((m) => ({
      id: Number(m.id),
      personId: m.person_id,
      body: m.body,
      fromMe: Boolean(m.from_me),
      createdAt: String(m.created_at),
    })),
    likesToday: Number(counts[0]?.n ?? 0),
    likeLimit: FREE_DAILY_LIKES,
  };
}

export const loadBoulevard = createServerFn({ method: "GET" })
  .middleware([authMiddleware])
  .handler(async ({ context }) => readSnap(context.userId));

export const saveProfile = createServerFn({ method: "POST" })
  .middleware([authMiddleware])
  .validator((input: unknown) => profileInput.parse(input))
  .handler(async ({ context, data }) => {
    const sql = await getSql();
    const greek = data.greek === "Independent" ? "" : data.greek;
    await sql`
      insert into boulevard_profiles (
        user_id, display_name, year_label, major, greek, intent, looking_for, bio
      ) values (
        ${context.userId},
        ${data.displayName},
        ${data.year},
        ${data.major},
        ${greek},
        ${data.intent},
        ${data.lookingFor},
        ${data.bio}
      )
      on conflict (user_id) do update set
        display_name = excluded.display_name,
        year_label = excluded.year_label,
        major = excluded.major,
        greek = excluded.greek,
        intent = excluded.intent,
        looking_for = excluded.looking_for,
        bio = excluded.bio,
        updated_at = now()
    `;
    return readSnap(context.userId);
  });

const swipeInput = z.object({
  personId: z.string(),
  direction: z.enum(["like", "pass"]),
});

export type SwipeResult = {
  ok: boolean;
  reason?: "limit" | "unknown" | "need-profile";
  matched: boolean;
  snap: BoulevardSnap;
};

export const swipePerson = createServerFn({ method: "POST" })
  .middleware([authMiddleware])
  .validator((input: unknown) => swipeInput.parse(input))
  .handler(async ({ context, data }): Promise<SwipeResult> => {
    const userId = context.userId;
    if (!PERSON_IDS.has(data.personId)) {
      return { ok: false, reason: "unknown", matched: false, snap: await readSnap(userId) };
    }
    const before = await readSnap(userId);
    if (!before.profile) {
      return { ok: false, reason: "need-profile", matched: false, snap: before };
    }
    const already = before.swipes.find((s) => s.personId === data.personId);
    if (already) {
      const person = personById(data.personId);
      return {
        ok: true,
        matched: already.direction === "like" && Boolean(person?.likesYou),
        snap: before,
      };
    }
    if (data.direction === "like" && !before.plus && before.likesToday >= before.likeLimit) {
      return { ok: false, reason: "limit", matched: false, snap: before };
    }

    const sql = await getSql();
    await sql`
      insert into boulevard_swipes (user_id, person_id, direction)
      values (${userId}, ${data.personId}, ${data.direction})
      on conflict (user_id, person_id) do nothing
    `;

    const person = personById(data.personId);
    const matched = data.direction === "like" && Boolean(person?.likesYou);
    if (matched && person?.note) {
      await sql`
        insert into boulevard_messages (user_id, person_id, body, from_me)
        select ${userId}, ${person.id}, ${person.note}, false
        where not exists (
          select 1 from boulevard_messages
          where user_id = ${userId} and person_id = ${person.id} and from_me = false
        )
      `;
    }

    return { ok: true, matched, snap: await readSnap(userId) };
  });

const messageInput = z.object({
  personId: z.string(),
  body: z.string().trim().min(1).max(500),
});

export const sendMessage = createServerFn({ method: "POST" })
  .middleware([authMiddleware])
  .validator((input: unknown) => messageInput.parse(input))
  .handler(async ({ context, data }) => {
    const userId = context.userId;
    const person = PEOPLE.find((p) => p.id === data.personId);
    const snap = await readSnap(userId);
    const liked = snap.swipes.some((s) => s.personId === data.personId && s.direction === "like");
    if (!person || !liked || !person.likesYou) {
      return { ok: false as const, snap };
    }
    const sql = await getSql();
    await sql`
      insert into boulevard_messages (user_id, person_id, body, from_me)
      values (${userId}, ${data.personId}, ${data.body}, true)
    `;
    return { ok: true as const, snap: await readSnap(userId) };
  });

export const setPlus = createServerFn({ method: "POST" })
  .middleware([authMiddleware])
  .validator((input: unknown) => z.object({ active: z.boolean() }).parse(input))
  .handler(async ({ context, data }) => {
    const sql = await getSql();
    await sql`
      insert into boulevard_plus (user_id, active)
      values (${context.userId}, ${data.active})
      on conflict (user_id) do update set active = excluded.active, updated_at = now()
    `;
    return readSnap(context.userId);
  });
