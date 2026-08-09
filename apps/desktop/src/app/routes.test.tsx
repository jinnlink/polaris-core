import { render, screen } from "@testing-library/react";

import { RoutePlaceholder } from "../components/RoutePlaceholder";
import { routeDefinitions } from "./routes";

describe("desktop route foundation", () => {
  it("registers every frozen P17A route exactly once", () => {
    expect(routeDefinitions.map(({ key }) => key)).toEqual([
      "today",
      "map",
      "practice",
      "inbox",
      "profile",
      "goals",
      "reports",
      "trust",
      "settings",
    ]);
    expect(new Set(routeDefinitions.map(({ path }) => path)).size).toBe(routeDefinitions.length);
  });

  it("gives route shells an accessible heading and honest empty state", () => {
    render(
      <RoutePlaceholder
        eyebrow="today"
        title="Today"
        description="今天的行动。"
      />,
    );

    expect(screen.getByRole("heading", { name: "Today" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("产品底座已就绪");
  });
});
