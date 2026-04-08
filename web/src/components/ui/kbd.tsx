import type { ComponentProps } from "solid-js";
import { splitProps } from "solid-js";
import { cn } from "@/lib/cn";

const Kbd = (props: ComponentProps<"kbd">) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <kbd
      class={cn(
        "pointer-events-none z-kbd inline-flex select-none items-center justify-center",
        local.class,
      )}
      data-slot="kbd"
      {...others}
    />
  );
};

export { Kbd };
