# JNA + UniFFI bindings are reached reflectively.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class dev.labdex.labterm.core.** { *; }
-dontwarn java.awt.*
