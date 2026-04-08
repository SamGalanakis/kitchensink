import { type ComponentProps, mergeProps, splitProps } from "solid-js";
import { cn } from "@/lib/cn";

type CardProps = ComponentProps<"div"> & { size?: "default" | "sm" };

const Card = (props: CardProps) => {
  const mergedProps = mergeProps({ size: "default" } as const, props);
  const [local, others] = splitProps(mergedProps, ["class", "size"]);
  return (
    <div
      data-slot="card"
      data-size={local.size}
      class={cn("group/card z-card flex flex-col", local.class)}
      {...others}
    />
  );
};

const CardHeader = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <div
      data-slot="card-header"
      class={cn(
        "group/card-header z-card-header grid auto-rows-min items-start has-data-[slot=card-action]:grid-cols-[1fr_auto] has-data-[slot=card-description]:grid-rows-[auto_auto]",
        local.class,
      )}
      {...others}
    />
  );
};

const CardTitle = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return <div data-slot="card-title" class={cn("z-card-title", local.class)} {...others} />;
};

const CardDescription = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return <div data-slot="card-description" class={cn("z-card-description", local.class)} {...others} />;
};

const CardAction = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <div
      data-slot="card-action"
      class={cn("z-card-action col-start-2 row-span-2 row-start-1 self-start justify-self-end", local.class)}
      {...others}
    />
  );
};

const CardContent = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return <div data-slot="card-content" class={cn("z-card-content", local.class)} {...others} />;
};

const CardFooter = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return (
    <div data-slot="card-footer" class={cn("z-card-footer flex items-center", local.class)} {...others} />
  );
};

export { Card, CardHeader, CardFooter, CardTitle, CardAction, CardDescription, CardContent };
