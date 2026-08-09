import { useEffect } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";

import { router } from "./app/routes";
import { installCoreEventRefresh } from "./lib/events";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
    },
  },
});

function DesktopRouter() {
  useEffect(() => {
    let cancelled = false;
    let dispose: (() => void) | undefined;
    void installCoreEventRefresh(queryClient).then((unlisten) => {
      if (cancelled) {
        unlisten();
      } else {
        dispose = unlisten;
      }
    });
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, []);

  return <RouterProvider router={router} />;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <DesktopRouter />
    </QueryClientProvider>
  );
}
