//===============================================================================
// #65	Gondoline Oliphant
//===============================================================================

class  WCOliphant extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardOliphantTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Oliphantsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardOliphantBigTexture FILE=TEXTURES\menu\Folio\Cards\Oliphantbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Gondoline Oliphant";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=65
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardOliphantTex0'
	textureBig=Texture'WizCardOliphantBigTexture'
	strDescriptionId="WizCard_0088"
}
