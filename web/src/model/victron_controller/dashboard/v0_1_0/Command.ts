// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {ChargeBatteryExtendedMode, ChargeBatteryExtendedMode_UEBACodec} from './ChargeBatteryExtendedMode'
import {DebugFullCharge, DebugFullCharge_UEBACodec} from './DebugFullCharge'
import {ForecastDisagreementStrategy, ForecastDisagreementStrategy_UEBACodec} from './ForecastDisagreementStrategy'
import {DischargeTime, DischargeTime_UEBACodec} from './DischargeTime'

export type Command = SetBoolKnob | SetFloatKnob | SetUintKnob | SetDischargeTime | SetDebugFullCharge | SetForecastDisagreementStrategy | SetChargeBatteryExtendedMode | SetKillSwitch

export const Command = {
    BaboonDomainVersion: '0.1.0',
    BaboonDomainIdentifier: 'victron_controller.dashboard',
    BaboonTypeIdentifier: 'victron_controller.dashboard/:#Command',
    BaboonSameInVersions: ["0.1.0", "0.2.0"],
    BaboonAdtTypeIdentifier: 'victron_controller.dashboard/:#Command',
    binCodec(): Command_UEBACodec {
        return Command_UEBACodec.instance
    }
} as const

export function isSetBoolKnob(value: Command): value is SetBoolKnob { return value instanceof SetBoolKnob; }
export function isSetFloatKnob(value: Command): value is SetFloatKnob { return value instanceof SetFloatKnob; }
export function isSetUintKnob(value: Command): value is SetUintKnob { return value instanceof SetUintKnob; }
export function isSetDischargeTime(value: Command): value is SetDischargeTime { return value instanceof SetDischargeTime; }
export function isSetDebugFullCharge(value: Command): value is SetDebugFullCharge { return value instanceof SetDebugFullCharge; }
export function isSetForecastDisagreementStrategy(value: Command): value is SetForecastDisagreementStrategy { return value instanceof SetForecastDisagreementStrategy; }
export function isSetChargeBatteryExtendedMode(value: Command): value is SetChargeBatteryExtendedMode { return value instanceof SetChargeBatteryExtendedMode; }
export function isSetKillSwitch(value: Command): value is SetKillSwitch { return value instanceof SetKillSwitch; }

export class SetBoolKnob implements BaboonGenerated {
    private readonly _knob_name: string;
    private readonly _value: boolean;

    constructor(knob_name: string, value: boolean) {
        this._knob_name = knob_name
        this._value = value
    }

    public get knob_name(): string {
        return this._knob_name;
    }
    public get value(): boolean {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            knob_name: this._knob_name,
            value: this._value
        };
    }

    public with(overrides: {knob_name?: string; value?: boolean}): SetBoolKnob {
        return new SetBoolKnob(
            'knob_name' in overrides ? overrides.knob_name! : this._knob_name,
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {knob_name: string; value: boolean}): SetBoolKnob {
        return new SetBoolKnob(
            obj.knob_name,
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetBoolKnob.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetBoolKnob.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetBoolKnob'
    public baboonTypeIdentifier() {
        return SetBoolKnob.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetBoolKnob.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetBoolKnob'
    public baboonAdtTypeIdentifier() {
        return SetBoolKnob.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetBoolKnob_UEBACodec {
        return SetBoolKnob_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetBoolKnob_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetBoolKnob, writer: BaboonBinWriter): unknown {
        if (this !== SetBoolKnob_UEBACodec.lazyInstance.value) {
          return SetBoolKnob_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.knob_name);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeBool(buffer, value.value);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.knob_name);
            BinTools.writeBool(writer, value.value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetBoolKnob {
        if (this !== SetBoolKnob_UEBACodec .lazyInstance.value) {
            return SetBoolKnob_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const knob_name = BinTools.readString(reader);
        const value = BinTools.readBool(reader);
        return new SetBoolKnob(
            knob_name,
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetBoolKnob_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetBoolKnob_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetBoolKnob'
    public baboonTypeIdentifier() {
        return SetBoolKnob_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetBoolKnob'
    public baboonAdtTypeIdentifier() {
        return SetBoolKnob_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetBoolKnob_UEBACodec())
    public static get instance(): SetBoolKnob_UEBACodec {
        return SetBoolKnob_UEBACodec.lazyInstance.value
    }
}

export class SetFloatKnob implements BaboonGenerated {
    private readonly _knob_name: string;
    private readonly _value: number;

    constructor(knob_name: string, value: number) {
        this._knob_name = knob_name
        this._value = value
    }

    public get knob_name(): string {
        return this._knob_name;
    }
    public get value(): number {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            knob_name: this._knob_name,
            value: this._value
        };
    }

    public with(overrides: {knob_name?: string; value?: number}): SetFloatKnob {
        return new SetFloatKnob(
            'knob_name' in overrides ? overrides.knob_name! : this._knob_name,
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {knob_name: string; value: number}): SetFloatKnob {
        return new SetFloatKnob(
            obj.knob_name,
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetFloatKnob.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetFloatKnob.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetFloatKnob'
    public baboonTypeIdentifier() {
        return SetFloatKnob.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetFloatKnob.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetFloatKnob'
    public baboonAdtTypeIdentifier() {
        return SetFloatKnob.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetFloatKnob_UEBACodec {
        return SetFloatKnob_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetFloatKnob_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetFloatKnob, writer: BaboonBinWriter): unknown {
        if (this !== SetFloatKnob_UEBACodec.lazyInstance.value) {
          return SetFloatKnob_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.knob_name);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeF64(buffer, value.value);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.knob_name);
            BinTools.writeF64(writer, value.value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetFloatKnob {
        if (this !== SetFloatKnob_UEBACodec .lazyInstance.value) {
            return SetFloatKnob_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const knob_name = BinTools.readString(reader);
        const value = BinTools.readF64(reader);
        return new SetFloatKnob(
            knob_name,
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetFloatKnob_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetFloatKnob_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetFloatKnob'
    public baboonTypeIdentifier() {
        return SetFloatKnob_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetFloatKnob'
    public baboonAdtTypeIdentifier() {
        return SetFloatKnob_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetFloatKnob_UEBACodec())
    public static get instance(): SetFloatKnob_UEBACodec {
        return SetFloatKnob_UEBACodec.lazyInstance.value
    }
}

export class SetUintKnob implements BaboonGenerated {
    private readonly _knob_name: string;
    private readonly _value: number;

    constructor(knob_name: string, value: number) {
        this._knob_name = knob_name
        this._value = value
    }

    public get knob_name(): string {
        return this._knob_name;
    }
    public get value(): number {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            knob_name: this._knob_name,
            value: this._value
        };
    }

    public with(overrides: {knob_name?: string; value?: number}): SetUintKnob {
        return new SetUintKnob(
            'knob_name' in overrides ? overrides.knob_name! : this._knob_name,
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {knob_name: string; value: number}): SetUintKnob {
        return new SetUintKnob(
            obj.knob_name,
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetUintKnob.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetUintKnob.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetUintKnob'
    public baboonTypeIdentifier() {
        return SetUintKnob.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetUintKnob.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetUintKnob'
    public baboonAdtTypeIdentifier() {
        return SetUintKnob.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetUintKnob_UEBACodec {
        return SetUintKnob_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetUintKnob_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetUintKnob, writer: BaboonBinWriter): unknown {
        if (this !== SetUintKnob_UEBACodec.lazyInstance.value) {
          return SetUintKnob_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.knob_name);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI32(buffer, value.value);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.knob_name);
            BinTools.writeI32(writer, value.value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetUintKnob {
        if (this !== SetUintKnob_UEBACodec .lazyInstance.value) {
            return SetUintKnob_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const knob_name = BinTools.readString(reader);
        const value = BinTools.readI32(reader);
        return new SetUintKnob(
            knob_name,
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetUintKnob_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetUintKnob_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetUintKnob'
    public baboonTypeIdentifier() {
        return SetUintKnob_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetUintKnob'
    public baboonAdtTypeIdentifier() {
        return SetUintKnob_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetUintKnob_UEBACodec())
    public static get instance(): SetUintKnob_UEBACodec {
        return SetUintKnob_UEBACodec.lazyInstance.value
    }
}

export class SetDischargeTime implements BaboonGenerated {
    private readonly _value: DischargeTime;

    constructor(value: DischargeTime) {
        this._value = value
    }

    public get value(): DischargeTime {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            value: this._value
        };
    }

    public with(overrides: {value?: DischargeTime}): SetDischargeTime {
        return new SetDischargeTime(
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {value: DischargeTime}): SetDischargeTime {
        return new SetDischargeTime(
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetDischargeTime.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetDischargeTime.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDischargeTime'
    public baboonTypeIdentifier() {
        return SetDischargeTime.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetDischargeTime.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDischargeTime'
    public baboonAdtTypeIdentifier() {
        return SetDischargeTime.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetDischargeTime_UEBACodec {
        return SetDischargeTime_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetDischargeTime_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetDischargeTime, writer: BaboonBinWriter): unknown {
        if (this !== SetDischargeTime_UEBACodec.lazyInstance.value) {
          return SetDischargeTime_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            DischargeTime_UEBACodec.instance.encode(ctx, value.value, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            DischargeTime_UEBACodec.instance.encode(ctx, value.value, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetDischargeTime {
        if (this !== SetDischargeTime_UEBACodec .lazyInstance.value) {
            return SetDischargeTime_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = DischargeTime_UEBACodec.instance.decode(ctx, reader);
        return new SetDischargeTime(
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetDischargeTime_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetDischargeTime_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDischargeTime'
    public baboonTypeIdentifier() {
        return SetDischargeTime_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDischargeTime'
    public baboonAdtTypeIdentifier() {
        return SetDischargeTime_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetDischargeTime_UEBACodec())
    public static get instance(): SetDischargeTime_UEBACodec {
        return SetDischargeTime_UEBACodec.lazyInstance.value
    }
}

export class SetDebugFullCharge implements BaboonGenerated {
    private readonly _value: DebugFullCharge;

    constructor(value: DebugFullCharge) {
        this._value = value
    }

    public get value(): DebugFullCharge {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            value: this._value
        };
    }

    public with(overrides: {value?: DebugFullCharge}): SetDebugFullCharge {
        return new SetDebugFullCharge(
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {value: DebugFullCharge}): SetDebugFullCharge {
        return new SetDebugFullCharge(
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetDebugFullCharge.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetDebugFullCharge.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDebugFullCharge'
    public baboonTypeIdentifier() {
        return SetDebugFullCharge.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetDebugFullCharge.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDebugFullCharge'
    public baboonAdtTypeIdentifier() {
        return SetDebugFullCharge.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetDebugFullCharge_UEBACodec {
        return SetDebugFullCharge_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetDebugFullCharge_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetDebugFullCharge, writer: BaboonBinWriter): unknown {
        if (this !== SetDebugFullCharge_UEBACodec.lazyInstance.value) {
          return SetDebugFullCharge_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            DebugFullCharge_UEBACodec.instance.encode(ctx, value.value, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            DebugFullCharge_UEBACodec.instance.encode(ctx, value.value, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetDebugFullCharge {
        if (this !== SetDebugFullCharge_UEBACodec .lazyInstance.value) {
            return SetDebugFullCharge_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = DebugFullCharge_UEBACodec.instance.decode(ctx, reader);
        return new SetDebugFullCharge(
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetDebugFullCharge_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetDebugFullCharge_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDebugFullCharge'
    public baboonTypeIdentifier() {
        return SetDebugFullCharge_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetDebugFullCharge'
    public baboonAdtTypeIdentifier() {
        return SetDebugFullCharge_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetDebugFullCharge_UEBACodec())
    public static get instance(): SetDebugFullCharge_UEBACodec {
        return SetDebugFullCharge_UEBACodec.lazyInstance.value
    }
}

export class SetForecastDisagreementStrategy implements BaboonGenerated {
    private readonly _value: ForecastDisagreementStrategy;

    constructor(value: ForecastDisagreementStrategy) {
        this._value = value
    }

    public get value(): ForecastDisagreementStrategy {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            value: this._value
        };
    }

    public with(overrides: {value?: ForecastDisagreementStrategy}): SetForecastDisagreementStrategy {
        return new SetForecastDisagreementStrategy(
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {value: ForecastDisagreementStrategy}): SetForecastDisagreementStrategy {
        return new SetForecastDisagreementStrategy(
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetForecastDisagreementStrategy.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetForecastDisagreementStrategy.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetForecastDisagreementStrategy'
    public baboonTypeIdentifier() {
        return SetForecastDisagreementStrategy.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetForecastDisagreementStrategy.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetForecastDisagreementStrategy'
    public baboonAdtTypeIdentifier() {
        return SetForecastDisagreementStrategy.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetForecastDisagreementStrategy_UEBACodec {
        return SetForecastDisagreementStrategy_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetForecastDisagreementStrategy_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetForecastDisagreementStrategy, writer: BaboonBinWriter): unknown {
        if (this !== SetForecastDisagreementStrategy_UEBACodec.lazyInstance.value) {
          return SetForecastDisagreementStrategy_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            ForecastDisagreementStrategy_UEBACodec.instance.encode(ctx, value.value, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            ForecastDisagreementStrategy_UEBACodec.instance.encode(ctx, value.value, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetForecastDisagreementStrategy {
        if (this !== SetForecastDisagreementStrategy_UEBACodec .lazyInstance.value) {
            return SetForecastDisagreementStrategy_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = ForecastDisagreementStrategy_UEBACodec.instance.decode(ctx, reader);
        return new SetForecastDisagreementStrategy(
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetForecastDisagreementStrategy_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetForecastDisagreementStrategy_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetForecastDisagreementStrategy'
    public baboonTypeIdentifier() {
        return SetForecastDisagreementStrategy_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetForecastDisagreementStrategy'
    public baboonAdtTypeIdentifier() {
        return SetForecastDisagreementStrategy_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetForecastDisagreementStrategy_UEBACodec())
    public static get instance(): SetForecastDisagreementStrategy_UEBACodec {
        return SetForecastDisagreementStrategy_UEBACodec.lazyInstance.value
    }
}

export class SetChargeBatteryExtendedMode implements BaboonGenerated {
    private readonly _value: ChargeBatteryExtendedMode;

    constructor(value: ChargeBatteryExtendedMode) {
        this._value = value
    }

    public get value(): ChargeBatteryExtendedMode {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            value: this._value
        };
    }

    public with(overrides: {value?: ChargeBatteryExtendedMode}): SetChargeBatteryExtendedMode {
        return new SetChargeBatteryExtendedMode(
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {value: ChargeBatteryExtendedMode}): SetChargeBatteryExtendedMode {
        return new SetChargeBatteryExtendedMode(
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetChargeBatteryExtendedMode.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetChargeBatteryExtendedMode.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetChargeBatteryExtendedMode'
    public baboonTypeIdentifier() {
        return SetChargeBatteryExtendedMode.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetChargeBatteryExtendedMode.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetChargeBatteryExtendedMode'
    public baboonAdtTypeIdentifier() {
        return SetChargeBatteryExtendedMode.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetChargeBatteryExtendedMode_UEBACodec {
        return SetChargeBatteryExtendedMode_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetChargeBatteryExtendedMode_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetChargeBatteryExtendedMode, writer: BaboonBinWriter): unknown {
        if (this !== SetChargeBatteryExtendedMode_UEBACodec.lazyInstance.value) {
          return SetChargeBatteryExtendedMode_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            ChargeBatteryExtendedMode_UEBACodec.instance.encode(ctx, value.value, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            ChargeBatteryExtendedMode_UEBACodec.instance.encode(ctx, value.value, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetChargeBatteryExtendedMode {
        if (this !== SetChargeBatteryExtendedMode_UEBACodec .lazyInstance.value) {
            return SetChargeBatteryExtendedMode_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = ChargeBatteryExtendedMode_UEBACodec.instance.decode(ctx, reader);
        return new SetChargeBatteryExtendedMode(
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetChargeBatteryExtendedMode_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetChargeBatteryExtendedMode_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetChargeBatteryExtendedMode'
    public baboonTypeIdentifier() {
        return SetChargeBatteryExtendedMode_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetChargeBatteryExtendedMode'
    public baboonAdtTypeIdentifier() {
        return SetChargeBatteryExtendedMode_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetChargeBatteryExtendedMode_UEBACodec())
    public static get instance(): SetChargeBatteryExtendedMode_UEBACodec {
        return SetChargeBatteryExtendedMode_UEBACodec.lazyInstance.value
    }
}

export class SetKillSwitch implements BaboonGenerated {
    private readonly _value: boolean;

    constructor(value: boolean) {
        this._value = value
    }

    public get value(): boolean {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            value: this._value
        };
    }

    public with(overrides: {value?: boolean}): SetKillSwitch {
        return new SetKillSwitch(
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {value: boolean}): SetKillSwitch {
        return new SetKillSwitch(
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetKillSwitch.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetKillSwitch.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetKillSwitch'
    public baboonTypeIdentifier() {
        return SetKillSwitch.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return SetKillSwitch.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetKillSwitch'
    public baboonAdtTypeIdentifier() {
        return SetKillSwitch.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SetKillSwitch_UEBACodec {
        return SetKillSwitch_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class SetKillSwitch_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SetKillSwitch, writer: BaboonBinWriter): unknown {
        if (this !== SetKillSwitch_UEBACodec.lazyInstance.value) {
          return SetKillSwitch_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeBool(buffer, value.value);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeBool(writer, value.value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SetKillSwitch {
        if (this !== SetKillSwitch_UEBACodec .lazyInstance.value) {
            return SetKillSwitch_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = BinTools.readBool(reader);
        return new SetKillSwitch(
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SetKillSwitch_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SetKillSwitch_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetKillSwitch'
    public baboonTypeIdentifier() {
        return SetKillSwitch_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#Command]#SetKillSwitch'
    public baboonAdtTypeIdentifier() {
        return SetKillSwitch_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SetKillSwitch_UEBACodec())
    public static get instance(): SetKillSwitch_UEBACodec {
        return SetKillSwitch_UEBACodec.lazyInstance.value
    }
}


/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class Command_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Command, writer: BaboonBinWriter): unknown {
        if (this !== Command_UEBACodec.lazyInstance.value) {
          return Command_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (value instanceof SetBoolKnob) {
                BinTools.writeByte(writer, 0);
                SetBoolKnob_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetFloatKnob) {
                BinTools.writeByte(writer, 1);
                SetFloatKnob_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetUintKnob) {
                BinTools.writeByte(writer, 2);
                SetUintKnob_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetDischargeTime) {
                BinTools.writeByte(writer, 3);
                SetDischargeTime_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetDebugFullCharge) {
                BinTools.writeByte(writer, 4);
                SetDebugFullCharge_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetForecastDisagreementStrategy) {
                BinTools.writeByte(writer, 5);
                SetForecastDisagreementStrategy_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetChargeBatteryExtendedMode) {
                BinTools.writeByte(writer, 6);
                SetChargeBatteryExtendedMode_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SetKillSwitch) {
                BinTools.writeByte(writer, 7);
                SetKillSwitch_UEBACodec.instance.encode(ctx, value, writer);
            }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Command {
        if (this !== Command_UEBACodec .lazyInstance.value) {
            return Command_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return SetBoolKnob_UEBACodec.instance.decode(ctx, reader)
                case 1: return SetFloatKnob_UEBACodec.instance.decode(ctx, reader)
                case 2: return SetUintKnob_UEBACodec.instance.decode(ctx, reader)
                case 3: return SetDischargeTime_UEBACodec.instance.decode(ctx, reader)
                case 4: return SetDebugFullCharge_UEBACodec.instance.decode(ctx, reader)
                case 5: return SetForecastDisagreementStrategy_UEBACodec.instance.decode(ctx, reader)
                case 6: return SetChargeBatteryExtendedMode_UEBACodec.instance.decode(ctx, reader)
                case 7: return SetKillSwitch_UEBACodec.instance.decode(ctx, reader)
            default: throw new Error("Unknown ADT branch tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Command_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Command_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Command'
    public baboonTypeIdentifier() {
        return Command_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/:#Command'
    public baboonAdtTypeIdentifier() {
        return Command_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Command_UEBACodec())
    public static get instance(): Command_UEBACodec {
        return Command_UEBACodec.lazyInstance.value
    }
}