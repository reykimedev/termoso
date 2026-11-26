import { Box, Typography } from "@mui/material";
import { tr } from "@/i18n";

export function LogoMark({ size = 32 }: { size?: number }) {
  return (
    <Box
      component="svg"
      viewBox="0 0 32 32"
      sx={{ width: size, height: size, flexShrink: 0 }}
      aria-hidden
    >
      {/* labdex's lantern with a prompt, line version: generated from labdex brand.py */}
      <rect width="32" height="32" rx="7" fill="#141413" />
      <rect
        x=".5"
        y=".5"
        width="31"
        height="31"
        rx="6.5"
        fill="none"
        stroke="#fff"
        strokeOpacity=".1"
      />
      <path
        d="M13.2 7.6a2.8 2.8 0 0 1 5.6 0"
        fill="none"
        stroke="#f59a23"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path d="M11.4 8.2h9.2l2.4 3H9z" fill="#f59a23" />
      <path
        d="M10.2 11.2h11.6v12.3a1.8 1.8 0 0 1-1.8 1.8h-8a1.8 1.8 0 0 1-1.8-1.8z"
        fill="none"
        stroke="#f59a23"
        strokeWidth="2"
      />
      <path
        d="M12.9 15.3l2.7 2.7-2.7 2.7"
        fill="none"
        stroke="#f59a23"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M16.9 20.7h2.2"
        fill="none"
        stroke="#f59a23"
        strokeWidth="1.9"
        strokeLinecap="round"
      />
      <path d="M8.6 25.6h14.8" stroke="#f59a23" strokeWidth="2.2" strokeLinecap="round" />
    </Box>
  );
}

export function Logo({ size = 32, showName = true }: { size?: number; showName?: boolean }) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1.25 }}>
      <LogoMark size={size} />
      {showName && (
        <Typography
          variant="h6"
          component="span"
          sx={{ fontWeight: 700, letterSpacing: "-0.02em", lineHeight: 1 }}
        >
          {tr("Labterm")}
        </Typography>
      )}
    </Box>
  );
}
