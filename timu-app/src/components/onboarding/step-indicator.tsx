import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useTheme } from '@/hooks/use-theme';
import { Fonts, Spacing } from '@/constants/theme';

type StepStatus = 'pending' | 'current' | 'completed' | 'error';

type Step = {
  label: string;
  status: StepStatus;
};

type StepIndicatorProps = {
  steps: Step[];
};

export function StepIndicator({ steps }: StepIndicatorProps) {
  const theme = useTheme();

  return (
    <View style={styles.container}>
      {steps.map((step, index) => {
        const isLast = index === steps.length - 1;
        return (
          <View key={index} style={styles.stepRow}>
            <View style={styles.leftColumn}>
              <View
                style={[
                  styles.circle,
                  {
                    backgroundColor:
                      step.status === 'completed'
                        ? theme.success
                        : step.status === 'error'
                          ? theme.danger
                          : step.status === 'current'
                            ? theme.primary
                            : theme.backgroundElement,
                    borderColor: step.status === 'pending' ? theme.border : 'transparent',
                    borderWidth: step.status === 'pending' ? 2 : 0,
                  },
                ]}>
                {step.status === 'completed' && (
                  <Text style={[styles.check, { color: theme.primaryText }]} numberOfLines={1}>
                    ✓
                  </Text>
                )}
                {step.status === 'error' && (
                  <Text style={[styles.check, { color: theme.primaryText }]} numberOfLines={1}>
                    ✕
                  </Text>
                )}
                {step.status === 'current' && (
                  <View style={[styles.pulseDot, { backgroundColor: theme.primaryText }]} />
                )}
              </View>

              {!isLast && (
                <View
                  style={[
                    styles.line,
                    {
                      backgroundColor:
                        step.status === 'completed' ? theme.success : theme.border,
                    },
                  ]}
                />
              )}
            </View>

            <Text
              style={[
                styles.label,
                {
                  color:
                    step.status === 'pending' ? theme.textSecondary : theme.text,
                },
              ]}
              numberOfLines={1}>
              {step.label}
            </Text>
          </View>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    gap: 0,
  },
  stepRow: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    minHeight: 44,
    gap: Spacing.three,
  },
  leftColumn: {
    width: 24,
    alignItems: 'center',
  },
  circle: {
    width: 24,
    height: 24,
    borderRadius: 12,
    alignItems: 'center',
    justifyContent: 'center',
  },
  pulseDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  check: {
    fontSize: 12,
    fontWeight: '700',
    fontFamily: Fonts.sans,
  },
  line: {
    width: 2,
    flex: 1,
    marginTop: Spacing.half,
  },
  label: {
    flex: 1,
    fontSize: 15,
    fontWeight: '500',
    lineHeight: 24,
    fontFamily: Fonts.sans,
  },
});
