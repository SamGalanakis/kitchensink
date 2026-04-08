import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "solid-js";
import { splitProps } from "solid-js";
import { cn } from "@/lib/cn";

const alertVariants = cva("group/alert relative z-alert w-full", {
  variants: {
    variant: {
      default: "z-alert-variant-default",
      destructive: "z-alert-variant-destructive",
    },
  },
  defaultVariants: {
    variant: "default",
  },
});

type AlertProps = ComponentProps<"div"> & VariantProps<typeof alertVariants>;

const Alert = (props: AlertProps) => {
  const [local, others] = splitProps(props, ["class", "variant"]);
  return (
    <div
      class={cn(alertVariants({ variant: local.variant }), local.class)}
      data-slot="alert"
      role="alert"
      {...others}
    />
  );
};

const AlertTitle = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return <div class={cn("z-alert-title", local.class)} data-slot="alert-title" {...others} />;
};

const AlertDescription = (props: ComponentProps<"div">) => {
  const [local, others] = splitProps(props, ["class"]);
  return <div class={cn("z-alert-description", local.class)} data-slot="alert-description" {...others} />;
};

export { Alert, AlertTitle, AlertDescription, alertVariants };
