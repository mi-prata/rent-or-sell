import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

/** Keeps one broken part of the page from blanking the rest. */
export default class ErrorBoundary extends Component<
  { children: ReactNode; what: string },
  { error: Error | null }
> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="card">
          <p className="muted small">
            The {this.props.what} could not be shown: {this.state.error.message}
          </p>
          <button type="button" className="more" onClick={() => this.setState({ error: null })}>
            Try again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
