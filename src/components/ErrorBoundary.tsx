import React from "react";

interface State {
  error?: Error;
}

export class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  State
> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = {};
  }

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("Erreur non gérée :", error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="error-boundary" role="alert">
          <h1>Une erreur est survenue</h1>
          <p>{this.state.error.message || String(this.state.error)}</p>
          <button onClick={() => this.setState({ error: undefined })}>
            Réessayer
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
