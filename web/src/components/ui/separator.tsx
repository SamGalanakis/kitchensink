import { type ComponentProps, splitProps } from "solid-js";
import { cn } from "@/lib/cn";

type SeparatorProps = ComponentProps<"hr"> & { orientation?: "horizontal" | "vertical" };

const Separator = (props: SeparatorProps) => {
  const [local, others] = splitProps(props, ["class", "orientation"]);
  const orient = () => local.orientation ?? "horizontal";
  return (
    <hr
      data-slot="separator"
      data-orientation={orient()}
      class={cn(
        "shrink-0 border-none bg-border",
        orient() === "horizontal" ? "h-px w-full" : "h-full w-px",
        local.class,
      )}
      {...others}
    />
  );
};

export { Separator };
