import { createFileRoute } from "@tanstack/react-router";
import { RedirectToSignIn } from "@/lib/auth/gates";
import { useCurrentUserState } from "@/lib/auth/use-current-user";
import { Boulevard } from "@/components/boulevard";

export const Route = createFileRoute("/")({ component: Home });

function Home() {
  const { user, isPending } = useCurrentUserState();
  if (isPending) return <Boot />;
  if (!user) return <RedirectToSignIn />;
  return <Boulevard />;
}

function Boot() {
  return (
    <main className="grid min-h-screen place-items-center bg-paper text-ink">
      <p className="font-display text-2xl">Boulevard</p>
    </main>
  );
}
