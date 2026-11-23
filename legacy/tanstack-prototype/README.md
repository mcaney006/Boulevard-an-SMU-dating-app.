# Boulevard

Verified SMU dating app. The order you see people in is a published score. Nothing else moves it.

## What is live

- Sign-in, then a profile: name, year, major, house, intent, who you want to see.
- Deck of fictional stand-in students so the product works before a live campus.
- Score on every card: same intent +40, same major +25, same house +20, year within one step +15.
- "Already liked you" (+15) is visible before you swipe only on Boulevard+.
- Free: 8 likes a day, intent and year filters, chat after a mutual.
- Boulevard+: unlimited likes, see who liked you, house and major filters. The preview unlock does not charge a card.
- Chats open only after a mutual like. Their note, if they left one, is the first message.

The eight people on the deck are not real students. Mutual likes from them are labeled as scripted stand-ins until matching runs for real.

## Repo

This repository started as a title. The product code that the running app uses:

- `src/lib/campus.ts` — the deck and the published rules
- `src/lib/rank.ts` — the score
- `src/lib/boulevard.fn.ts` — profile, swipes, messages, Plus, scoped to the signed-in user
- `src/components/boulevard.tsx` — the app
- `migrations/0002_boulevard.sql` — profiles, swipes, messages, Plus

Cloudflare (verification mail, live campus, billing) is not in this commit. That is the next step.
