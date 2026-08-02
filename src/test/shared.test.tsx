import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ErrorNote, StatusBadge } from "@/components/shared";

describe("ErrorNote", () => {
  it("shows the friendly message and expands technical details", async () => {
    const error = {
      category: "Authentication",
      detail: {
        user_message: "The API key was not accepted.",
        technical: "HTTP 401: access key verification failed",
      },
    };
    render(<ErrorNote error={error} />);
    expect(screen.getByRole("alert")).toHaveTextContent("The API key was not accepted.");
    expect(screen.queryByText(/HTTP 401/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /technical details/i }));
    expect(screen.getByText(/HTTP 401/)).toBeInTheDocument();
  });

  it("falls back to a generic message for unknown errors", () => {
    render(<ErrorNote error={{ weird: true }} />);
    expect(screen.getByRole("alert")).toHaveTextContent(/something went wrong/i);
  });
});

describe("StatusBadge", () => {
  it("renders text labels, not color alone", () => {
    render(
      <>
        <StatusBadge status="filled" />
        <StatusBadge status="rejected" />
        <StatusBadge status="partially_filled" />
      </>,
    );
    expect(screen.getByText("Filled")).toBeInTheDocument();
    expect(screen.getByText("Rejected")).toBeInTheDocument();
    expect(screen.getByText("Partially filled")).toBeInTheDocument();
  });
});
