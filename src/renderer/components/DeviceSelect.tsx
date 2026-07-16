import type { DeviceInfo } from '../types/protocol';
import type { Locale } from '../settings/types';
import { t } from '../settings/i18n';

interface DeviceSelectProps {
  devices: DeviceInfo[];
  activeDeviceId: string | null;
  onChange: (deviceId: string) => void;
  locale: Locale;
}

export function DeviceSelect({ devices, activeDeviceId, onChange, locale }: DeviceSelectProps) {
  if (devices.length === 0) {
    return (
      <div className="device-select">
        <select
          className="field__input device-select__control"
          disabled
          value=""
          aria-label={t(locale, 'device')}
        >
          <option value="">{t(locale, 'noDevices')}</option>
        </select>
      </div>
    );
  }

  return (
    <div className="device-select">
      <select
        className="field__input device-select__control"
        value={activeDeviceId ?? devices[0]?.deviceId ?? ''}
        onChange={(event) => onChange(event.currentTarget.value)}
        aria-label={t(locale, 'device')}
      >
        {devices.map((device) => (
          <option key={device.deviceId} value={device.deviceId}>
            {device.deviceName} · {device.deviceId} ({device.source.toUpperCase()})
          </option>
        ))}
      </select>
    </div>
  );
}
