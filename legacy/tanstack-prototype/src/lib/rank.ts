import { type Intent, type Person, type Year, YEARS } from "@/lib/campus";

export type RankProfile = {
  intent: Intent;
  major: string;
  greek: string;
  year: Year;
};

export type Reason = { label: string; points: number };

const YEAR_INDEX = new Map(YEARS.map((y, i) => [y, i]));

export function scorePerson(
  me: RankProfile,
  them: Person,
  revealLike: boolean,
): { score: number; reasons: Reason[] } {
  const reasons: Reason[] = [];

  if (me.intent === them.intent) {
    reasons.push({ label: "Same intent", points: 40 });
  }
  if (me.major === them.major) {
    reasons.push({ label: "Same major", points: 25 });
  }
  const myGreek = me.greek && me.greek !== "Independent" ? me.greek : "";
  if (myGreek && them.greek && myGreek === them.greek) {
    reasons.push({ label: "Same house", points: 20 });
  }
  const a = YEAR_INDEX.get(me.year) ?? 0;
  const b = YEAR_INDEX.get(them.year) ?? 0;
  if (Math.abs(a - b) <= 1) {
    reasons.push({
      label: a === b ? "Same year" : "Adjacent year",
      points: 15,
    });
  }
  if (them.likesYou && revealLike) {
    reasons.push({ label: "Already liked you", points: 15 });
  }

  const score = reasons.reduce((sum, r) => sum + r.points, 0);
  return { score, reasons };
}
