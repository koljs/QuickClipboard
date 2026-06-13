import '@tabler/icons-webfont/dist/tabler-icons.min.css';
import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import SettingsSection from '../components/SettingsSection';
import SettingItem from '../components/SettingItem';
import Input from '@shared/components/ui/Input';
import Select from '@shared/components/ui/Select';
import Button from '@shared/components/ui/Button';
import { testAiTranslation } from '@shared/api/system';
import { toast, TOAST_SIZES, TOAST_POSITIONS } from '@shared/store/toastStore';
import i18n from '@shared/i18n';

const TOAST_CONFIG = {
  size: TOAST_SIZES.EXTRA_SMALL,
  position: TOAST_POSITIONS.BOTTOM_RIGHT
};

function AIConfigSection({
  settings,
  onSettingChange
}) {
  const {
    t
  } = useTranslation();
  const [testing, setTesting] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const modelOptions = [{
    value: 'Qwen/Qwen2-7B-Instruct',
    label: 'Qwen2-7B-Instruct (推荐)'
  }, {
    value: 'deepseek-v3',
    label: 'DeepSeek V3'
  }, {
    value: 'qwen-turbo',
    label: '通义千问 Turbo'
  }, {
    value: 'chatglm3-6b',
    label: 'ChatGLM3-6B'
  }, {
    value: 'yi-34b-chat',
    label: 'Yi-34B-Chat'
  }];
  const handleTestConfig = async () => {
    setTesting(true);
    try {
      await testAiTranslation();
      toast.success(i18n.t('settings.aiConfig.testSuccess'), TOAST_CONFIG);
    } catch (error) {
      toast.error(i18n.t('settings.aiConfig.testFailed') + ': ' + error, TOAST_CONFIG);
    } finally {
      setTesting(false);
    }
  };
  const handleRefreshModels = async () => {
    setRefreshing(true);
    setTimeout(() => setRefreshing(false), 1000);
  };
  return <SettingsSection title={t('settings.aiConfig.title')} description={t('settings.aiConfig.description')}>
      <SettingItem label={t('settings.aiConfig.apiKey')} description={t('settings.aiConfig.apiKeyDesc')}>
        <Input type="password" value={settings.aiApiKey || ''} onChange={e => onSettingChange('aiApiKey', e.target.value)} placeholder={t('settings.aiConfig.apiKeyPlaceholder')} className="w-80" />
      </SettingItem>

      <SettingItem label={t('settings.aiConfig.model')} description={t('settings.aiConfig.modelDesc')}>
        <div className="flex items-center gap-2">
          <Select value={settings.aiModel} onChange={value => onSettingChange('aiModel', value)} options={modelOptions} className="w-64" />
          <Button onClick={handleRefreshModels} loading={refreshing} variant="secondary" size="sm" icon={<i className="ti ti-refresh w-4 h-4"></i>} title={t('settings.aiConfig.refreshModels')} />
        </div>
      </SettingItem>

      <SettingItem label={t('settings.aiConfig.baseUrl')} description={t('settings.aiConfig.baseUrlDesc')}>
        <Input type="text" value={settings.aiBaseUrl || ''} onChange={e => onSettingChange('aiBaseUrl', e.target.value)} placeholder="https://api.siliconflow.cn/v1" className="w-80" />
      </SettingItem>

      <SettingItem label={t('settings.aiConfig.test')} description={t('settings.aiConfig.testDesc')}>
        <Button onClick={handleTestConfig} loading={testing} icon={<i className="ti ti-test-pipe"></i>}>
          {t('settings.aiConfig.testButton')}
        </Button>
      </SettingItem>
    </SettingsSection>;
}
export default AIConfigSection;