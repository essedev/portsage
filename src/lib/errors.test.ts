import { describe, expect, it } from "vitest";
import { humanizeError } from "./errors";

describe("humanizeError", () => {
  describe("SQLite UNIQUE constraints", () => {
    it("translates duplicate project name", () => {
      expect(humanizeError("UNIQUE constraint failed: projects.name")).toBe(
        "A project with this name already exists.",
      );
    });

    it("translates duplicate port", () => {
      expect(humanizeError("UNIQUE constraint failed: ports.port")).toBe(
        "This port is already assigned to another project.",
      );
    });
  });

  describe("port-outside-range custom error", () => {
    it("rephrases the message and reuses the captured numbers", () => {
      expect(humanizeError("port 9999 is outside project range 4000-4009")).toBe(
        "Port 9999 is outside this project's range (4000-4009).",
      );
    });

    it("works for arbitrary port and range values", () => {
      expect(humanizeError("port 5050 is outside project range 5000-5009")).toBe(
        "Port 5050 is outside this project's range (5000-5009).",
      );
    });
  });

  describe("SQLite contention", () => {
    it("translates database is locked", () => {
      expect(humanizeError("database is locked")).toBe(
        "The database is busy. Please try again in a moment.",
      );
    });
  });

  describe("filesystem errors", () => {
    it("translates No such file or directory", () => {
      expect(humanizeError("No such file or directory")).toBe("File not found.");
    });

    it("translates os error 2", () => {
      expect(humanizeError("open failed: os error 2")).toBe("File not found.");
    });

    it("translates Permission denied", () => {
      expect(humanizeError("Permission denied")).toBe(
        "Permission denied. Check the file permissions and try again.",
      );
    });

    it("translates os error 13", () => {
      expect(humanizeError("write failed: os error 13")).toBe(
        "Permission denied. Check the file permissions and try again.",
      );
    });
  });

  describe("socket-layer errors", () => {
    it("rephrases project not found and preserves the project name", () => {
      expect(humanizeError("project 'my-app' not found")).toBe(
        'Project "my-app" not found.',
      );
    });
  });

  describe("input normalization", () => {
    it("accepts plain strings", () => {
      expect(humanizeError("UNIQUE constraint failed: projects.name")).toBe(
        "A project with this name already exists.",
      );
    });

    it("accepts Error instances and reads the message", () => {
      expect(
        humanizeError(new Error("UNIQUE constraint failed: ports.port")),
      ).toBe("This port is already assigned to another project.");
    });

    it("falls back to JSON for unknown object shapes", () => {
      // Unknown shape: should not throw, returns the JSON as fallback.
      const result = humanizeError({ foo: "bar" });
      expect(result).toBe('{"foo":"bar"}');
    });

    it("returns 'Unknown error' when JSON serialization fails", () => {
      const cyclic: Record<string, unknown> = {};
      cyclic.self = cyclic;
      expect(humanizeError(cyclic)).toBe("Unknown error");
    });
  });

  describe("fallback behavior", () => {
    it("returns the raw text for unmapped errors instead of swallowing them", () => {
      const raw = "some completely unexpected error from the future";
      expect(humanizeError(raw)).toBe(raw);
    });

    it("does not partially match unrelated text containing one of the keywords", () => {
      // 'unique' alone (without 'constraint failed:') should NOT trigger the
      // duplicate-name pattern.
      const raw = "the user has a unique style";
      expect(humanizeError(raw)).toBe(raw);
    });
  });
});
