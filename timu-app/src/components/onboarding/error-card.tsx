import { Pressable, StyleSheet, Text, View } from 'react-native';
import type { ReactNode } from 'react';

import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

type ErrorCardProps = {
  title: string;
  message?: string;
  command?: string;
  actionTitle?: string;
  onAction?: () => void;
  severity?: 'error' | 'warning';
  children?: ReactNode;
};

export function ErrorCard({
  title,
  message,
  command,
  actionTitle,
  onAction,
  severity = 'error',
  children,
}: ErrorCardProps) {
  const theme = useTheme();
  const accent = severity === 'error' ? theme.danger : theme.warning;

  return (
    <View
      style={[
        styles.card,
        {
          backgroundColor: theme.backgroundElement,
          borderColor: accent,
          borderWidth: 1,
        },
      ]}>
      <View style={styles.header}>
        <View style={[styles.badge, { backgroundColor: accent }]} />
        <Text style={[styles.title, { color: theme.text }]} numberOfLines={2}>
          {title}
        </Text>
      </View>

      {message && (
        <Text style={[styles.message, { color: theme.textSecondary }]} numberOfLines={4}>
          {message}
        </Text>
      )}

      {command && (
        <View style={[styles.commandRow, { backgroundColor: theme.backgroundSelected }]}>
          <Text style={[styles.commandText, { color: theme.text }]} numberOfLines={1}>
            <Text style={{ color: accent }}>$</Text> {command}
          </Text>
          {actionTitle && (
            <Pressable onPress={onAction} style={styles.commandAction}>
              <Text style={[styles.commandActionText, { color: accent }]} numberOfLines={1}>
                {actionTitle}
              </Text>
            </Pressable>
          )}
        </View>
      )}

      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    borderRadius: 16,
    padding: Spacing.three,
    gap: Spacing.three,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: Spacing.two,
  },
  badge: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  title: {
    flex: 1,
    fontSize: 16,
    fontWeight: '600',
    fontFamily: Fonts.sans,
  },
  message: {
    fontSize: 14,
    lineHeight: 20,
    fontWeight: '400',
    fontFamily: Fonts.sans,
  },
  commandRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    borderRadius: 10,
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.two,
    gap: Spacing.two,
  },
  commandText: {
    flex: 1,
    fontSize: 14,
    fontWeight: '500',
    fontFamily: Fonts.mono,
  },
  commandAction: {
    paddingVertical: Spacing.half,
    paddingHorizontal: Spacing.two,
  },
  commandActionText: {
    fontSize: 14,
    fontWeight: '600',
    fontFamily: Fonts.sans,
  },
});
