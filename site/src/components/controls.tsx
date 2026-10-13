"use client";

/**
 * Shift's form controls (packages/ui in the Shift repo), on Base UI with the
 * same classes. Sizes are written in px because this site keeps Tailwind's
 * default text-sm/text-xs, which Shift overrides to 12px/10px.
 */

import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { Field } from "@base-ui/react/field";
import { Select as BaseSelect } from "@base-ui/react/select";
import { Slider as BaseSlider } from "@base-ui/react/slider";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import clsx from "clsx";
import { Check, ChevronDown } from "lucide-react";

const focus = "outline-none focus-visible:ring-1 focus-visible:ring-accent";

/** A label above a control. */
export function Labelled({ label, children }: { label: React.ReactNode; children: React.ReactNode }) {
  return (
    <Field.Root className="flex min-w-0 flex-col gap-1.5">
      <Field.Label className="font-ui text-[11px] text-muted">{label}</Field.Label>
      {children}
    </Field.Root>
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  className,
}: {
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
  className?: string;
}) {
  return (
    <BaseSelect.Root items={options} value={value} onValueChange={(v) => v !== null && onChange(v as T)}>
      <BaseSelect.Trigger
        className={clsx(
          "flex h-7 min-w-0 cursor-pointer items-center justify-between gap-2 rounded bg-input px-2 font-ui text-[12px] text-primary",
          focus,
          className,
        )}
      >
        <BaseSelect.Value />
        <BaseSelect.Icon className="shrink-0 text-muted">
          <ChevronDown className="size-3.5" />
        </BaseSelect.Icon>
      </BaseSelect.Trigger>
      <BaseSelect.Portal>
        <BaseSelect.Positioner className="z-50" sideOffset={4} alignItemWithTrigger={false}>
          <BaseSelect.Popup className="min-w-(--anchor-width) rounded-md border border-line-subtle bg-surface-raised p-1 shadow-lg outline-none">
            <BaseSelect.List>
              {options.map((o) => (
                <BaseSelect.Item
                  key={o.value}
                  value={o.value}
                  className="grid h-7 cursor-pointer grid-cols-[1rem_minmax(0,1fr)] items-center gap-2 rounded px-2 font-ui text-[12px] whitespace-nowrap text-primary outline-none select-none data-[highlighted]:bg-hover/50"
                >
                  <BaseSelect.ItemIndicator className="flex items-center justify-center">
                    <Check className="size-3.5" strokeWidth={1.75} />
                  </BaseSelect.ItemIndicator>
                  <BaseSelect.ItemText className="col-start-2">{o.label}</BaseSelect.ItemText>
                </BaseSelect.Item>
              ))}
            </BaseSelect.List>
          </BaseSelect.Popup>
        </BaseSelect.Positioner>
      </BaseSelect.Portal>
    </BaseSelect.Root>
  );
}

export function Slider({
  value,
  onChange,
  min,
  max,
  step,
  label,
}: {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  step: number;
  label: string;
}) {
  return (
    <BaseSlider.Root
      value={value}
      min={min}
      max={max}
      step={step}
      onValueChange={(v) => onChange(v as number)}
      thumbAlignment="center"
      className="relative flex h-7 w-full touch-none items-center select-none"
    >
      <BaseSlider.Control className="flex h-3.5 w-full cursor-pointer items-center px-1.75">
        <BaseSlider.Track className="relative h-1.5 w-full rounded-full bg-control-muted">
          <BaseSlider.Indicator className="absolute h-full rounded-full bg-accent" />
          <BaseSlider.Thumb
            aria-label={label}
            className="size-3.5 rounded-full border-2 border-primary bg-surface-raised shadow-sm focus-visible:ring-1 focus-visible:ring-accent focus-visible:outline-none"
          />
        </BaseSlider.Track>
      </BaseSlider.Control>
    </BaseSlider.Root>
  );
}

export function Checkbox({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children: React.ReactNode;
}) {
  return (
    <label className="flex h-7 cursor-pointer items-center gap-2 font-ui text-[12px] text-primary">
      <BaseCheckbox.Root
        checked={checked}
        onCheckedChange={onChange}
        className={clsx(
          "inline-flex size-3.5 shrink-0 cursor-pointer items-center justify-center rounded-sm border border-line-subtle bg-input text-primary data-[checked]:border-accent",
          focus,
        )}
      >
        <BaseCheckbox.Indicator>
          <Check className="size-3" strokeWidth={2.5} />
        </BaseCheckbox.Indicator>
      </BaseCheckbox.Root>
      {children}
    </label>
  );
}

/** A row of text toggles where pressing one releases the others. */
export function Segmented<T extends string>({
  value,
  onChange,
  options,
  label,
}: {
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
  label: string;
}) {
  return (
    <ToggleGroup
      aria-label={label}
      value={[value]}
      onValueChange={(v) => v[0] && onChange(v[0] as T)}
      className="flex h-7 items-center gap-0.5 rounded bg-input p-0.5"
    >
      {options.map((o) => (
        <Toggle
          key={o.value}
          value={o.value}
          className="inline-flex h-6 cursor-pointer items-center rounded-[3px] px-2 font-ui text-[12px] text-muted transition-colors hover:text-primary focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:outline-none data-[pressed]:bg-surface-raised data-[pressed]:text-primary data-[pressed]:shadow-sm"
        >
          {o.label}
        </Toggle>
      ))}
    </ToggleGroup>
  );
}
