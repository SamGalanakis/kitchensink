import { type ComponentProps, splitProps } from "solid-js";
import { cn } from "@/lib/cn";

type InputProps = ComponentProps<"input">;

const Input = (props: InputProps) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <input
      data-slot="input"
      class={cn(
        "z-input w-full min-w-0 outline-none placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
        local.class,
      )}
      {...others}
    />
  );
};

export { Input, type InputProps };
