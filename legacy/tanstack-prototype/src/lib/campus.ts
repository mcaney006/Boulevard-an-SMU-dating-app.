export const INTENTS = ["casual", "dating", "relationship"] as const;
export type Intent = (typeof INTENTS)[number];

export const YEARS = ["First-year", "Sophomore", "Junior", "Senior", "Graduate"] as const;
export type Year = (typeof YEARS)[number];

export const LOOKING = ["women", "men", "everyone"] as const;
export type Looking = (typeof LOOKING)[number];

export const MAJORS = [
  "Finance",
  "Marketing",
  "Computer Science",
  "Journalism",
  "Political Science",
  "Film",
  "Mechanical Engineering",
  "Economics",
] as const;

export const GREEKS = [
  "Independent",
  "Kappa Kappa Gamma",
  "Pi Beta Phi",
  "Delta Delta Delta",
  "Kappa Alpha Theta",
  "Sigma Chi",
  "Kappa Alpha",
  "Pi Kappa Alpha",
] as const;

export type Gender = "woman" | "man";

export type Person = {
  id: string;
  name: string;
  gender: Gender;
  year: Year;
  major: string;
  school: string;
  greek: string;
  intent: Intent;
  bio: string;
  photo: string;
  likesYou: boolean;
  note: string;
};

export const PEOPLE: Person[] = [
  {
    id: "ava",
    name: "Ava Chen",
    gender: "woman",
    year: "Junior",
    major: "Finance",
    school: "Cox",
    greek: "Kappa Kappa Gamma",
    intent: "dating",
    bio: "Cox junior who will leave a formal early if the conversation is better than the band. Looking for someone who can be direct.",
    photo: "/people/ava.jpg",
    likesYou: true,
    note: "Your major filter is close enough. If you're actually on campus this week, say so.",
  },
  {
    id: "maya",
    name: "Maya Ellis",
    gender: "woman",
    year: "Sophomore",
    major: "Film",
    school: "Meadows",
    greek: "",
    intent: "relationship",
    bio: "Independent, Meadows, allergic to 'let's just see.' I want a real plan, not a 1 a.m. porch.",
    photo: "/people/maya.jpg",
    likesYou: false,
    note: "",
  },
  {
    id: "elena",
    name: "Elena Vargas",
    gender: "woman",
    year: "Sophomore",
    major: "Marketing",
    school: "Cox",
    greek: "Pi Beta Phi",
    intent: "casual",
    bio: "Here for something light and specific. If your profile says relationship and your chats say otherwise, I'm gone.",
    photo: "/people/elena.jpg",
    likesYou: true,
    note: "Intent match. Don't open with a compliment about the photo.",
  },
  {
    id: "priya",
    name: "Priya Shah",
    gender: "woman",
    year: "Junior",
    major: "Political Science",
    school: "Dedman",
    greek: "Independent",
    intent: "dating",
    bio: "Dedman, debate, and a low tolerance for vague plans. Coffee on the Hilltop beats another app that hides the ranking.",
    photo: "/people/priya.jpg",
    likesYou: true,
    note: "I liked you because the intent line was dating, not a shrug.",
  },
  {
    id: "camille",
    name: "Camille Brooks",
    gender: "woman",
    year: "Senior",
    major: "Journalism",
    school: "Meadows",
    greek: "Kappa Alpha Theta",
    intent: "dating",
    bio: "Senior, leaving in May, not interested in a semester of almost. Tell me what you're actually looking for.",
    photo: "/people/camille.jpg",
    likesYou: false,
    note: "",
  },
  {
    id: "sofia",
    name: "Sofia Nguyen",
    gender: "woman",
    year: "First-year",
    major: "Economics",
    school: "Dedman",
    greek: "Delta Delta Delta",
    intent: "casual",
    bio: "First-year who already knows the group-chat version of everyone. Verified students only is the whole point.",
    photo: "/people/sofia.jpg",
    likesYou: false,
    note: "",
  },
  {
    id: "liam",
    name: "Liam Ortiz",
    gender: "man",
    year: "Senior",
    major: "Mechanical Engineering",
    school: "Lyle",
    greek: "Sigma Chi",
    intent: "casual",
    bio: "Lyle, Sigma Chi, done pretending the app is a personality test. Clear intent, then a real plan.",
    photo: "/people/liam.jpg",
    likesYou: true,
    note: "If you're in Lyle or just nearby, that's enough to start.",
  },
  {
    id: "ethan",
    name: "Ethan Park",
    gender: "man",
    year: "Junior",
    major: "Computer Science",
    school: "Lyle",
    greek: "",
    intent: "relationship",
    bio: "Independent, builds things, not interested in a deck that lies about why you showed up. Relationship or nothing.",
    photo: "/people/ethan.jpg",
    likesYou: false,
    note: "",
  },
];

export const PERSON_IDS = new Set(PEOPLE.map((p) => p.id));

export function personById(id: string): Person | undefined {
  return PEOPLE.find((p) => p.id === id);
}

export function intentLabel(intent: Intent): string {
  if (intent === "casual") return "Something casual";
  if (intent === "dating") return "Actually dating";
  return "A relationship";
}

export const FREE_DAILY_LIKES = 8;
export const PLUS_PRICE = "$12 / month";

export const RANK_RULES = [
  "Same intent: +40. This is a hard signal, not a suggestion.",
  "Same major: +25.",
  "Both in a house, same house: +20. One independent: no greek points.",
  "Year within one step: +15. Same year: +15. Further apart: +0.",
  "They already liked you: +15, shown before you swipe only on Boulevard+.",
  "Nothing else moves the order. No hidden score, no boost slot, no paid reroll.",
] as const;
