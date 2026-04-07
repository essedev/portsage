import { useState } from "react";
import { UIInput } from "@/components/ui/UIInput";
import { UIButton } from "@/components/ui/UIButton";

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
      <UIInput
        label="Project name"
        placeholder="e.g. my-project"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      <UIInput
        label="Path (optional)"
        placeholder="/Users/.../project"
        value={path}
        onChange={(e) => setPath(e.target.value)}
      />
      <div className="flex justify-end gap-[var(--spacing-2)]">
        <UIButton variant="ghost" type="button" onClick={onCancel}>
          Cancel
        </UIButton>
        <UIButton variant="primary" type="submit">
          Create project
        </UIButton>
      </div>
    </form>
  );
}
