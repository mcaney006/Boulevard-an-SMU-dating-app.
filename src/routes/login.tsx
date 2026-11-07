import { createFileRoute } from "@tanstack/react-router";
import { GROK_PROVIDERS, authEnabled, signIn } from "@/lib/auth/client";

export const Route = createFileRoute("/login")({ component: Login });

function Login() {
  return (
    <main className="grid min-h-screen place-items-center bg-paper px-6 text-ink">
      <div className="w-full max-w-sm">
        <p className="font-display text-sm tracking-wide text-crimson">Boulevard</p>
        <h1 className="mt-2 font-display text-4xl leading-none">Verified on the Hilltop.</h1>
        <p className="mt-3 text-sm leading-relaxed text-muted">
          SMU students only. The order you see people in is the published score, not a hidden one.
        </p>
        <div className="mt-8 flex flex-col gap-2">
          {authEnabled ? (
            GROK_PROVIDERS.map((p) => (
              <button
                key={p.providerId}
                type="button"
                onClick={() => signIn(p.providerId, { callbackURL: "/" })}
                className="h-12 rounded-full bg-ink px-4 text-sm font-medium text-paper"
              >
                Continue with {p.label}
              </button>
            ))
          ) : (
            <p className="text-sm text-muted">Sign-in is off.</p>
          )}
        </div>
      </div>
    </main>
  );
}
