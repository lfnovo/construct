import { Component, type ReactNode } from "react";
import { api } from "./api";
import type { TabMode } from "./types";

type Props = {
  children: ReactNode;
  mode: TabMode;
  onRequestSource: () => void;
  onRetry?: () => void;
};

export class DocumentErrorBoundary extends Component<Props, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch() {
    // Error messages/stacks can contain Markdown or private paths. Log only the mode.
    void api.reportDocumentRenderFailure(this.props.mode).catch(() => {});
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div className="review-error" role="alert">
        <div>
          <strong>This document view could not be rendered.</strong>
          <p>{this.props.mode === "source"
            ? "Your document buffer is still available. Use Save to save it, or retry this view."
            : "Your document buffer is still available. Open Source to continue editing or save it."}</p>
          {this.props.mode !== "source" && <button className="toolbar-button" onClick={this.props.onRequestSource}>Open Source</button>}
          <button className="toolbar-button" onClick={() => {
            this.setState({ failed: false });
            this.props.onRetry?.();
          }}>Retry view</button>
        </div>
      </div>
    );
  }
}
