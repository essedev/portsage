import { useState } from "react";
import { GrimInput } from "@/components/ui/GrimInput";
import { GrimButton } from "@/components/ui/GrimButton";

interface AddProjectFormProps {
  onSubmit: (name: string, path?: string) => void;
  onCancel: () => void;
}

export function AddProjectForm({ onSubmit, onCancel }: AddProjectFormProps) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name.trim()) {
      onSubmit(name.trim(), path.trim() || undefined);
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-[var(--spacing-2)] p-[var(--spacing-3)]"
    >
      <GrimInput
        label="Project name"
        placeholder="e.g. my-project"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      <GrimInput
        label="Path (optional)"
        placeholder="/Users/.../project"
        value={path}
        onChange={(e) => setPath(e.target.value)}
      />
      <div className="flex justify-end gap-[var(--spacing-2)]">
        <GrimButton variant="ghost" type="button" onClick={onCancel}>
          Cancel
        </GrimButton>
        <GrimButton variant="primary" type="submit">
          Create project
        </GrimButton>
      </div>
    </form>
  );
}
