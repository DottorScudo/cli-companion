import { Component } from "react";
import type { ReactNode } from "react";
import { Button } from "./ui/button";
import { errorMessage, report } from "../lib/errors";

interface Props {
  children: ReactNode;
}

interface State {
  error: unknown;
  failed: boolean;
}

/** Schermata di ripiego quando un componente lancia durante il render: l'app non resta bianca. */
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, failed: false };

  static getDerivedStateFromError(error: unknown): State {
    return { error, failed: true };
  }

  componentDidCatch(error: unknown) {
    report("rendering dell'interfaccia", error);
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div
        role="alert"
        className="flex h-screen flex-col items-center justify-center gap-3 bg-background p-6 text-center"
      >
        <h1 className="text-lg font-semibold">Qualcosa è andato storto</h1>
        <p className="max-w-md break-words text-sm text-muted-foreground">{errorMessage(this.state.error)}</p>
        <p className="max-w-md text-xs text-muted-foreground">
          I terminali già avviati e i dati salvati non sono stati toccati. Ricarica per riprovare.
        </p>
        <Button onClick={() => window.location.reload()}>Ricarica</Button>
      </div>
    );
  }
}
