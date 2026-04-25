// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class Timer implements BaboonGeneratedLatest {
    private readonly _id: string;
    private readonly _description: string;
    private readonly _period_ms: bigint;
    private readonly _last_fire_epoch_ms: bigint | undefined;
    private readonly _next_fire_epoch_ms: bigint | undefined;
    private readonly _status: string;

    constructor(id: string, description: string, period_ms: bigint, last_fire_epoch_ms: bigint | undefined, next_fire_epoch_ms: bigint | undefined, status: string) {
        this._id = id
        this._description = description
        this._period_ms = period_ms
        this._last_fire_epoch_ms = last_fire_epoch_ms
        this._next_fire_epoch_ms = next_fire_epoch_ms
        this._status = status
    }

    public get id(): string {
        return this._id;
    }
    public get description(): string {
        return this._description;
    }
    public get period_ms(): bigint {
        return this._period_ms;
    }
    public get last_fire_epoch_ms(): bigint | undefined {
        return this._last_fire_epoch_ms;
    }
    public get next_fire_epoch_ms(): bigint | undefined {
        return this._next_fire_epoch_ms;
    }
    public get status(): string {
        return this._status;
    }

    public toJSON(): Record<string, unknown> {
        return {
            id: this._id,
            description: this._description,
            period_ms: this._period_ms,
            last_fire_epoch_ms: this._last_fire_epoch_ms !== undefined ? this._last_fire_epoch_ms : undefined,
            next_fire_epoch_ms: this._next_fire_epoch_ms !== undefined ? this._next_fire_epoch_ms : undefined,
            status: this._status
        };
    }

    public with(overrides: {id?: string; description?: string; period_ms?: bigint; last_fire_epoch_ms?: bigint | undefined; next_fire_epoch_ms?: bigint | undefined; status?: string}): Timer {
        return new Timer(
            'id' in overrides ? overrides.id! : this._id,
            'description' in overrides ? overrides.description! : this._description,
            'period_ms' in overrides ? overrides.period_ms! : this._period_ms,
            'last_fire_epoch_ms' in overrides ? overrides.last_fire_epoch_ms! : this._last_fire_epoch_ms,
            'next_fire_epoch_ms' in overrides ? overrides.next_fire_epoch_ms! : this._next_fire_epoch_ms,
            'status' in overrides ? overrides.status! : this._status
        );
    }

    public static fromPlain(obj: {id: string; description: string; period_ms: bigint; last_fire_epoch_ms: bigint | undefined; next_fire_epoch_ms: bigint | undefined; status: string}): Timer {
        return new Timer(
            obj.id,
            obj.description,
            obj.period_ms,
            obj.last_fire_epoch_ms,
            obj.next_fire_epoch_ms,
            obj.status
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Timer.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Timer.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Timer'
    public baboonTypeIdentifier() {
        return Timer.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0"]
    public baboonSameInVersions() {
        return Timer.BaboonSameInVersions
    }
    public static binCodec(): Timer_UEBACodec {
        return Timer_UEBACodec.instance
    }
}

export class Timer_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Timer, writer: BaboonBinWriter): unknown {
        if (this !== Timer_UEBACodec.lazyInstance.value) {
          return Timer_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.id);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.description);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI64(buffer, value.period_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.last_fire_epoch_ms === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeI64(buffer, value.last_fire_epoch_ms);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.next_fire_epoch_ms === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeI64(buffer, value.next_fire_epoch_ms);
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
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.id);
            BinTools.writeString(writer, value.description);
            BinTools.writeI64(writer, value.period_ms);
            if (value.last_fire_epoch_ms === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeI64(writer, value.last_fire_epoch_ms);
            }
            if (value.next_fire_epoch_ms === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeI64(writer, value.next_fire_epoch_ms);
            }
            BinTools.writeString(writer, value.status);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Timer {
        if (this !== Timer_UEBACodec .lazyInstance.value) {
            return Timer_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 5; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const id = BinTools.readString(reader);
        const description = BinTools.readString(reader);
        const period_ms = BinTools.readI64(reader);
        const last_fire_epoch_ms = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readI64(reader));
        const next_fire_epoch_ms = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readI64(reader));
        const status = BinTools.readString(reader);
        return new Timer(
            id,
            description,
            period_ms,
            last_fire_epoch_ms,
            next_fire_epoch_ms,
            status,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Timer_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Timer_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Timer'
    public baboonTypeIdentifier() {
        return Timer_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Timer_UEBACodec())
    public static get instance(): Timer_UEBACodec {
        return Timer_UEBACodec.lazyInstance.value
    }
}