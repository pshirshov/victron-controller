// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class PinnedRegister implements BaboonGeneratedLatest {
    private readonly _path: string;
    private readonly _target_value_str: string;
    private readonly _current_value_str: string | undefined;
    private readonly _status: string;
    private readonly _drift_count: number;
    private readonly _last_drift_iso: string | undefined;
    private readonly _last_check_iso: string | undefined;

    constructor(path: string, target_value_str: string, current_value_str: string | undefined, status: string, drift_count: number, last_drift_iso: string | undefined, last_check_iso: string | undefined) {
        this._path = path
        this._target_value_str = target_value_str
        this._current_value_str = current_value_str
        this._status = status
        this._drift_count = drift_count
        this._last_drift_iso = last_drift_iso
        this._last_check_iso = last_check_iso
    }

    public get path(): string {
        return this._path;
    }
    public get target_value_str(): string {
        return this._target_value_str;
    }
    public get current_value_str(): string | undefined {
        return this._current_value_str;
    }
    public get status(): string {
        return this._status;
    }
    public get drift_count(): number {
        return this._drift_count;
    }
    public get last_drift_iso(): string | undefined {
        return this._last_drift_iso;
    }
    public get last_check_iso(): string | undefined {
        return this._last_check_iso;
    }

    public toJSON(): Record<string, unknown> {
        return {
            path: this._path,
            target_value_str: this._target_value_str,
            current_value_str: this._current_value_str !== undefined ? this._current_value_str : undefined,
            status: this._status,
            drift_count: this._drift_count,
            last_drift_iso: this._last_drift_iso !== undefined ? this._last_drift_iso : undefined,
            last_check_iso: this._last_check_iso !== undefined ? this._last_check_iso : undefined
        };
    }

    public with(overrides: {path?: string; target_value_str?: string; current_value_str?: string | undefined; status?: string; drift_count?: number; last_drift_iso?: string | undefined; last_check_iso?: string | undefined}): PinnedRegister {
        return new PinnedRegister(
            'path' in overrides ? overrides.path! : this._path,
            'target_value_str' in overrides ? overrides.target_value_str! : this._target_value_str,
            'current_value_str' in overrides ? overrides.current_value_str! : this._current_value_str,
            'status' in overrides ? overrides.status! : this._status,
            'drift_count' in overrides ? overrides.drift_count! : this._drift_count,
            'last_drift_iso' in overrides ? overrides.last_drift_iso! : this._last_drift_iso,
            'last_check_iso' in overrides ? overrides.last_check_iso! : this._last_check_iso
        );
    }

    public static fromPlain(obj: {path: string; target_value_str: string; current_value_str: string | undefined; status: string; drift_count: number; last_drift_iso: string | undefined; last_check_iso: string | undefined}): PinnedRegister {
        return new PinnedRegister(
            obj.path,
            obj.target_value_str,
            obj.current_value_str,
            obj.status,
            obj.drift_count,
            obj.last_drift_iso,
            obj.last_check_iso
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return PinnedRegister.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return PinnedRegister.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#PinnedRegister'
    public baboonTypeIdentifier() {
        return PinnedRegister.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return PinnedRegister.BaboonSameInVersions
    }
    public static binCodec(): PinnedRegister_UEBACodec {
        return PinnedRegister_UEBACodec.instance
    }
}

export class PinnedRegister_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: PinnedRegister, writer: BaboonBinWriter): unknown {
        if (this !== PinnedRegister_UEBACodec.lazyInstance.value) {
          return PinnedRegister_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.path);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.target_value_str);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.current_value_str === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.current_value_str);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.status);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI32(buffer, value.drift_count);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.last_drift_iso === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.last_drift_iso);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.last_check_iso === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.last_check_iso);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.path);
            BinTools.writeString(writer, value.target_value_str);
            if (value.current_value_str === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.current_value_str);
            }
            BinTools.writeString(writer, value.status);
            BinTools.writeI32(writer, value.drift_count);
            if (value.last_drift_iso === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.last_drift_iso);
            }
            if (value.last_check_iso === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.last_check_iso);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): PinnedRegister {
        if (this !== PinnedRegister_UEBACodec .lazyInstance.value) {
            return PinnedRegister_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 6; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const path = BinTools.readString(reader);
        const target_value_str = BinTools.readString(reader);
        const current_value_str = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const status = BinTools.readString(reader);
        const drift_count = BinTools.readI32(reader);
        const last_drift_iso = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const last_check_iso = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        return new PinnedRegister(
            path,
            target_value_str,
            current_value_str,
            status,
            drift_count,
            last_drift_iso,
            last_check_iso,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return PinnedRegister_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return PinnedRegister_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#PinnedRegister'
    public baboonTypeIdentifier() {
        return PinnedRegister_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new PinnedRegister_UEBACodec())
    public static get instance(): PinnedRegister_UEBACodec {
        return PinnedRegister_UEBACodec.lazyInstance.value
    }
}